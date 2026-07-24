from __future__ import annotations

import difflib
import json
from collections import Counter
from pathlib import Path

from rich.console import Console
from rich.markup import escape
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

from .header import identity_fits_header
from .models import FileResult, Identity


class Reporter:
    def __init__(
        self,
        *,
        output_format: str,
        no_color: bool,
        verbose: bool,
        show_diff: bool,
        cwd: Path,
    ) -> None:
        self.output_format = output_format
        self.verbose = verbose
        self.show_diff = show_diff
        self.cwd = cwd
        self.console = Console(
            no_color=no_color,
            force_terminal=False if no_color else None,
            highlight=False,
        )

    def render(
        self,
        results: list[FileResult],
        *,
        identity: Identity,
        discovery_failures: list[str],
        check_mode: bool,
    ) -> None:
        if self.output_format == "json":
            payload = {
                "mode": "check" if check_mode else "fix",
                "identity": {
                    "login": identity.login,
                    "email": identity.email,
                    "source": identity.source,
                    "inferred": identity.inferred,
                    "available": identity.available,
                },
                "discovery_errors": discovery_failures,
                "files": [result.to_dict() for result in results],
                "summary": self._summary(results),
            }
            self.console.print_json(json.dumps(payload))
            return

        self.console.print(
            Panel.fit(
                "[bold cyan]norminette-fix[/bold cyan]\n"
                "[dim]Safe automatic fixes for the 42 Norm v4.1[/dim]",
                border_style="cyan",
            )
        )
        if not identity.available:
            self.console.print(
                "[bold red]Official header not added:[/bold red] "
                "no verified 42 student email is available."
            )
            self.console.print(
                f"[dim]{escape(identity.source)}. Save it in "
                "~/.config/norminette-fix/config.ini or pass --email.[/dim]"
            )
        elif identity.inferred:
            self.console.print(
                "[yellow]Header identity was inferred[/yellow]: "
                f"{escape(identity.login)} <{escape(identity.email)}> "
                f"[dim]from {escape(identity.source)}[/dim]"
            )
            self.console.print(
                "[dim]Use --login and --email (or NORMINETTE_FIX_LOGIN / "
                "NORMINETTE_FIX_EMAIL) to override it.[/dim]"
            )
        if not identity_fits_header(identity):
            self.console.print(
                "[bold red]Official header not added:[/bold red] the verified 42 email "
                "does not fit the fixed 80-column template without truncation."
            )
            self.console.print(
                "[dim]The tool will never shorten or falsify the author email.[/dim]"
            )
        if discovery_failures:
            for failure in discovery_failures:
                self.console.print(f"[bold red]Input error:[/bold red] {escape(failure)}")

        table = Table(title="Files", show_lines=False)
        table.add_column("Status", no_wrap=True)
        table.add_column("File", overflow="fold")
        table.add_column("Fixes", justify="right")
        table.add_column("Remaining", justify="right")
        for result in results:
            if result.failure:
                status = "[bold red]FAILED[/bold red]"
            elif result.diagnostics_after:
                status = "[bold yellow]REVIEW[/bold yellow]"
            elif result.changed and result.wrote:
                status = "[bold green]FIXED[/bold green]"
            elif result.changed:
                status = "[bold blue]WOULD FIX[/bold blue]"
            else:
                status = "[green]CLEAN[/green]"
            table.add_row(
                status,
                escape(self._display_path(result.path)),
                str(sum(fix.count for fix in result.fixes)),
                str(len(result.diagnostics_after)),
            )
        self.console.print(table)

        if self.verbose:
            self._render_fixes(results)
        self._render_reviews(results)
        self._render_failures(results)
        if self.show_diff:
            self._render_diffs(results)
        self._render_summary(results, check_mode=check_mode)

    def _render_fixes(self, results: list[FileResult]) -> None:
        for result in results:
            if not result.fixes:
                continue
            counts = Counter((fix.code, fix.description) for fix in result.fixes)
            table = Table(
                title=f"Applied fixes - {escape(self._display_path(result.path))}",
                show_header=True,
            )
            table.add_column("Rule")
            table.add_column("Description")
            table.add_column("Count", justify="right")
            for (code, description), count in sorted(counts.items()):
                table.add_row(code, description, str(count))
            self.console.print(table)

    def _render_reviews(self, results: list[FileResult]) -> None:
        reviews = [
            (result, diagnostic) for result in results for diagnostic in result.diagnostics_after
        ]
        if not reviews:
            return
        self.console.print("\n[bold yellow]Manual attention required[/bold yellow]")
        for result in results:
            if not result.diagnostics_after:
                continue
            self.console.print(f"\n[bold]{escape(self._display_path(result.path))}[/bold]")
            for diagnostic in result.diagnostics_after:
                location = f"L{diagnostic.line}:C{diagnostic.column}"
                level_style = "yellow" if diagnostic.level == "Notice" else "bold yellow"
                self.console.print(
                    Text.assemble(
                        (f"  {location:<11}", "dim"),
                        (f"{diagnostic.code:<25}", level_style),
                        diagnostic.message,
                    )
                )
                if diagnostic.detail:
                    self.console.print(f"  {'':<11}[white]{escape(diagnostic.detail)}[/white]")
                if diagnostic.suggestion:
                    self.console.print(
                        f"  {'':<11}[dim]Next:[/dim] {escape(diagnostic.suggestion)}"
                    )
                if diagnostic.source != "norminette":
                    self.console.print(f"  {'':<11}[dim]Source: {escape(diagnostic.source)}[/dim]")

    def _render_failures(self, results: list[FileResult]) -> None:
        for result in results:
            if result.failure:
                self.console.print(
                    f"\n[bold red]FAILED[/bold red] "
                    f"{escape(self._display_path(result.path))}: "
                    f"{escape(result.failure)}"
                )

    def _render_diffs(self, results: list[FileResult]) -> None:
        for result in results:
            if not result.changed or result.original is None or result.fixed is None:
                continue
            path = self._display_path(result.path)
            diff_path = path.lstrip("/")
            diff = "".join(
                difflib.unified_diff(
                    result.original.splitlines(keepends=True),
                    result.fixed.splitlines(keepends=True),
                    fromfile=f"a/{diff_path}",
                    tofile=f"b/{diff_path}",
                )
            )
            if diff:
                # A diff must remain byte-for-byte applicable. Rich syntax
                # rendering expands tabs and may crop 81-column +/- lines.
                self.console.file.write(diff)
                self.console.file.flush()

    def _render_summary(self, results: list[FileResult], *, check_mode: bool) -> None:
        summary = self._summary(results)
        action = "would be fixed" if check_mode else "changed"
        message = (
            f"[bold]{summary['files']}[/bold] file(s) scanned  |  "
            f"[green]{summary['changed']}[/green] {action}  |  "
            f"[cyan]{summary['fixes']}[/cyan] fix(es)  |  "
            f"[yellow]{summary['remaining']}[/yellow] remaining issue(s)  |  "
            f"[red]{summary['failed']}[/red] failed"
        )
        if summary["failed"]:
            border = "red"
        elif summary["remaining"]:
            border = "yellow"
        else:
            border = "green"
        self.console.print(Panel(message, title="Summary", border_style=border))
        backups = [result.backup for result in results if result.backup]
        if backups:
            common = Path(
                str(Path(backups[0]).parents[1])
                if len(backups) == 1
                else str(Path(backups[0]).parents[1])
            )
            self.console.print(f"[dim]Backups: {escape(str(common))}[/dim]")

    @staticmethod
    def _summary(results: list[FileResult]) -> dict[str, int]:
        return {
            "files": len(results),
            "changed": sum(result.changed for result in results),
            "written": sum(result.wrote for result in results),
            "fixes": sum(len(result.fixes) for result in results),
            "remaining": sum(len(result.diagnostics_after) for result in results),
            "failed": sum(result.failure is not None for result in results),
        }

    def _display_path(self, path: Path) -> str:
        try:
            return str(path.relative_to(self.cwd))
        except ValueError:
            return str(path)
