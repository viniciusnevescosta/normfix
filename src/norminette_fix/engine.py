from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import tempfile
from collections import Counter
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from .analysis import enrich_diagnostics, supplemental_diagnostics
from .comments import remove_invalid_function_comments
from .guard_scope import (
    GuardApproval,
    advance_guard_approvals_after_write,
    guard_approval_is_current,
    plan_header_guard_renames,
)
from .header import (
    HeaderGuardRename,
    ensure_header,
    ensure_header_guard,
    header_filename_matches,
    update_header,
)
from .line_compaction import compact_continuation_lines
from .makefile import analyze_makefile, format_makefile, is_makefile
from .models import Diagnostic, FileResult, Fix, Highlight, Identity
from .norminette_adapter import NorminetteAdapter
from .source import normalize_hygiene
from .transforms import choose_phase, format_preprocessors


@dataclass(frozen=True)
class EngineOptions:
    write: bool = True
    backup: bool = True
    backup_root: Path | None = None
    max_passes: int = 100
    norminette_timeout: float = 5.0
    remove_invalid_comments: bool = False


class BackupManager:
    def __init__(self, root: Path | None = None) -> None:
        run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        base = root or (
            Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share"))
            / "norminette-fix"
            / "backups"
        )
        self.run_id = run_id
        self.root = base / run_id
        self._entries: list[dict[str, str | int]] = []

    def save(self, path: Path, original: bytes) -> Path:
        parent_hash = hashlib.sha256(str(path.parent).encode()).hexdigest()[:12]
        destination = self.root / parent_hash / path.name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(original)
        shutil.copystat(path, destination, follow_symlinks=False)
        self._entries.append(
            {
                "source": str(path),
                "backup": str(destination),
                "sha256": hashlib.sha256(original).hexdigest(),
                "mode": stat.S_IMODE(path.stat().st_mode),
            }
        )
        return destination

    def finish(self) -> None:
        if not self._entries:
            return
        manifest = {
            "run_id": self.run_id,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "files": self._entries,
        }
        self.root.mkdir(parents=True, exist_ok=True)
        (self.root / "manifest.json").write_text(
            json.dumps(manifest, indent=2) + "\n",
            encoding="utf-8",
        )


class FixEngine:
    def __init__(
        self,
        *,
        identity: Identity,
        options: EngineOptions,
        adapter: NorminetteAdapter | None = None,
    ) -> None:
        self.identity = identity
        self.options = options
        self.adapter = adapter or NorminetteAdapter(timeout_seconds=options.norminette_timeout)
        self.backups = (
            BackupManager(options.backup_root) if options.write and options.backup else None
        )
        self._guard_renames: dict[Path, GuardApproval] = {}

    def process(self, paths: list[Path]) -> list[FileResult]:
        try:
            self._guard_renames = plan_header_guard_renames(paths)
            results = [self.process_file(path) for path in paths]
            if self.backups:
                self.backups.finish()
            return results
        finally:
            self._guard_renames = {}

    def process_file(self, path: Path) -> FileResult:
        result = FileResult(path=path)
        if path.is_symlink():
            result.failure = "Refused to edit a symbolic link."
            return result
        try:
            original_bytes = path.read_bytes()
        except OSError as exc:
            result.failure = f"Could not read the file: {exc}"
            return result
        if b"\0" in original_bytes:
            result.failure = "Refused to process a binary file containing NUL bytes."
            return result
        try:
            original = original_bytes.decode("utf-8")
        except UnicodeDecodeError as exc:
            result.failure = f"The file is not valid UTF-8: {exc}"
            return result
        result.original = original
        if is_makefile(path):
            return self._process_makefile(result, path, original_bytes, original)

        before, lint_failure = self.adapter.lint(path, original)
        result.diagnostics_before = before
        if lint_failure:
            failure_diagnostic = self._failure_diagnostic(path, lint_failure)
            result.diagnostics_before = [failure_diagnostic]
            current = original
            if current.startswith("\ufeff"):
                current = current[1:]
                result.fixes.append(
                    Fix(
                        "REMOVE_BOM",
                        "removed the UTF-8 byte-order mark before the official header",
                        1,
                    )
                )
            current, header_changed, header_inserted = ensure_header(
                current,
                path.name,
                self.identity,
            )
            if header_changed:
                result.fixes.append(
                    Fix(
                        "INVALID_HEADER",
                        "inserted or repaired the official 42 header",
                        1,
                    )
                )
            if not header_inserted and not header_filename_matches(current, path.name):
                current, updated = update_header(current, path.name, self.identity)
                if updated:
                    result.fixes.append(
                        Fix(
                            "UPDATE_HEADER",
                            "updated the official header filename and modification metadata",
                            9,
                        )
                    )
            result.fixed = current
            result.diagnostics_after = enrich_diagnostics(
                [failure_diagnostic, *supplemental_diagnostics(path, current)],
                current,
                path,
            )
            self._write_if_changed(result, path, original_bytes, current)
            return result

        current = original
        current, hygiene = normalize_hygiene(current)
        result.fixes.extend(Fix(code, description, line) for code, description, line in hygiene)

        current, header_changed, header_inserted = ensure_header(current, path.name, self.identity)
        if header_changed:
            result.fixes.append(
                Fix(
                    "INVALID_HEADER",
                    "inserted or repaired the official 42 header",
                    1,
                )
            )

        if path.suffix == ".h":
            approval = self._guard_renames.get(path.resolve())
            approved_rename: HeaderGuardRename | None = None
            if approval is not None and guard_approval_is_current(approval):
                approved_rename = approval.rename
            current, guard_changed, guard = ensure_header_guard(
                current,
                path.name,
                approved_rename=approved_rename,
            )
            if guard_changed:
                assert approved_rename is not None
                result.fixes.append(
                    Fix(
                        "HEADER_PROTECTION",
                        (
                            f"renamed the {approved_rename.current} header guard "
                            f"to {guard} after a project-wide reference check"
                        ),
                        13,
                    )
                )

        preprocessed, preprocessor_edits = format_preprocessors(current)
        if preprocessor_edits and self._same_tokens(path, current, preprocessed):
            current = preprocessed
            result.fixes.extend(
                Fix(edit.code, edit.description, edit.line) for edit in preprocessor_edits
            )

        seen = {hashlib.sha256(current.encode()).hexdigest()}
        unstable = False
        for _ in range(self.options.max_passes):
            diagnostics, failure = self.adapter.lint(path, current)
            if failure:
                break
            if self.options.remove_invalid_comments:
                candidate, edits = remove_invalid_function_comments(current, diagnostics)
                invalid_comment_codes = {
                    diagnostic.code
                    for diagnostic in diagnostics
                    if diagnostic.code in {"WRONG_SCOPE_COMMENT", "COMMENT_ON_INSTR"}
                }
                if (
                    edits
                    and candidate != current
                    and self._same_code_tokens(path, current, candidate)
                ):
                    candidate_diagnostics, candidate_failure = self.adapter.lint(path, candidate)
                    if not candidate_failure and self._diagnostics_improve(
                        diagnostics,
                        candidate_diagnostics,
                        invalid_comment_codes,
                    ):
                        digest = hashlib.sha256(candidate.encode()).hexdigest()
                        if digest in seen:
                            unstable = True
                            break
                        seen.add(digest)
                        current = candidate
                        result.fixes.extend(
                            Fix(edit.code, edit.description, edit.line) for edit in edits
                        )
                        continue
            candidate, edits = compact_continuation_lines(current)
            if (
                edits
                and candidate != current
                and self._same_tokens(path, current, candidate)
            ):
                candidate_diagnostics, candidate_failure = self.adapter.lint(path, candidate)
                if not candidate_failure and self._diagnostics_improve(
                    diagnostics,
                    candidate_diagnostics,
                    set(),
                ):
                    digest = hashlib.sha256(candidate.encode()).hexdigest()
                    if digest in seen:
                        unstable = True
                        break
                    seen.add(digest)
                    current = candidate
                    result.fixes.extend(
                        Fix(edit.code, edit.description, edit.line) for edit in edits
                    )
                    continue
            candidate, edits, preserves_tokens = choose_phase(current, diagnostics)
            if not edits or candidate == current:
                break
            if preserves_tokens and not self._same_tokens(path, current, candidate):
                break
            guarded_codes = {edit.code for edit in edits if edit.code == "MISALIGNED_VAR_DECL"}
            if any(
                edit.code == "TOO_MANY_TAB" and "Norminette-guided probe" in edit.description
                for edit in edits
            ):
                guarded_codes.add("TOO_MANY_TAB")
            if guarded_codes:
                candidate_diagnostics, candidate_failure = self.adapter.lint(path, candidate)
                if candidate_failure or not self._diagnostics_improve(
                    diagnostics,
                    candidate_diagnostics,
                    guarded_codes,
                ):
                    break
            if not preserves_tokens:
                _, candidate_failure = self.adapter.lint(path, candidate)
                if candidate_failure:
                    break
            digest = hashlib.sha256(candidate.encode()).hexdigest()
            if digest in seen:
                unstable = True
                break
            seen.add(digest)
            current = candidate
            result.fixes.extend(Fix(edit.code, edit.description, edit.line) for edit in edits)
        else:
            diagnostics, failure = self.adapter.lint(path, current)
            if not failure:
                candidate, edits, _ = choose_phase(current, diagnostics)
                unstable = bool(edits and candidate != current)

        if unstable:
            result.fixed = original
            result.fixes.clear()
            result.failure = (
                "Automatic fixes did not reach a stable result; the original "
                "file was preserved to prevent an edit cycle."
            )
            result.diagnostics_after = enrich_diagnostics(
                [*before, *supplemental_diagnostics(path, original)],
                original,
                path,
            )
            return result

        current, final_hygiene = normalize_hygiene(current)
        result.fixes.extend(
            Fix(code, description, line) for code, description, line in final_hygiene
        )

        if (
            current != original or not header_filename_matches(current, path.name)
        ) and not header_inserted:
            current, updated = update_header(current, path.name, self.identity)
            if updated:
                result.fixes.append(
                    Fix(
                        "UPDATE_HEADER",
                        "updated the official header filename and modification metadata",
                        9,
                    )
                )

        result.fixed = current
        after, lint_failure = self.adapter.lint(path, current)
        if lint_failure:
            result.fixed = original
            result.fixes.clear()
            result.failure = (
                "Safety validation rejected the proposed edits because a parseable "
                "file became unparseable; the original was preserved."
            )
            after = [self._failure_diagnostic(path, lint_failure)]
            current = original
        after.extend(supplemental_diagnostics(path, current))
        result.diagnostics_after = enrich_diagnostics(after, current, path)
        if current == original:
            result.fixes.clear()

        self._write_if_changed(result, path, original_bytes, current)
        return result

    def _process_makefile(
        self,
        result: FileResult,
        path: Path,
        original_bytes: bytes,
        original: str,
    ) -> FileResult:
        result.diagnostics_before = analyze_makefile(path, original)
        current, fixes = format_makefile(original, path, self.identity)
        result.fixed = current
        result.fixes.extend(fixes)
        result.diagnostics_after = analyze_makefile(path, current)
        if current == original:
            result.fixes.clear()
        self._write_if_changed(result, path, original_bytes, current)
        return result

    def _write_if_changed(
        self,
        result: FileResult,
        path: Path,
        original_bytes: bytes,
        source: str,
    ) -> None:
        if not self.options.write or not result.changed:
            return
        try:
            if path.read_bytes() != original_bytes:
                result.failure = (
                    "The file changed in another program while it was being fixed; "
                    "no write was performed."
                )
                return
            if self.backups:
                result.backup = self.backups.save(path, original_bytes)
            if any(fix.code == "HEADER_PROTECTION" for fix in result.fixes):
                approval = self._guard_renames.get(path.resolve())
                if approval is None or not guard_approval_is_current(approval):
                    result.failure = (
                        "The project changed during the header-guard safety check; "
                        "no write was performed."
                    )
                    return
            self._atomic_write(path, source)
            result.wrote = True
            self._guard_renames = advance_guard_approvals_after_write(
                self._guard_renames,
                path,
            )
        except OSError as exc:
            result.failure = f"Could not safely write the file: {exc}"

    def _same_tokens(self, path: Path, before: str, after: str) -> bool:
        try:
            return self.adapter.token_fingerprint(path, before) == self.adapter.token_fingerprint(
                path, after
            )
        except Exception:
            return False

    def _same_code_tokens(self, path: Path, before: str, after: str) -> bool:
        try:
            return self.adapter.code_token_fingerprint(
                path,
                before,
            ) == self.adapter.code_token_fingerprint(path, after)
        except Exception:
            return False

    @staticmethod
    def _diagnostics_improve(
        before: list[Diagnostic],
        after: list[Diagnostic],
        guarded_codes: set[str],
    ) -> bool:
        before_counts = Counter(item.code for item in before)
        after_counts = Counter(item.code for item in after)
        if len(after) > len(before):
            return False
        for code in guarded_codes:
            if code == "TOO_MANY_TAB":
                if after_counts[code] > before_counts[code]:
                    return False
            elif after_counts[code] >= before_counts[code]:
                return False
        return not any(
            code not in guarded_codes and count > before_counts[code]
            for code, count in after_counts.items()
        )

    @staticmethod
    def _failure_diagnostic(path: Path, message: str) -> Diagnostic:
        return Diagnostic(
            code="PARSER_FAILURE",
            message=message,
            level="Error",
            path=path,
            highlights=(Highlight(1, 1),),
            suggestion=(
                "Repair the reported C syntax or lexical error first; automatic "
                "formatting cannot safely reason about an unparseable file."
            ),
            source="norminette-fix",
        )

    @staticmethod
    def _atomic_write(path: Path, source: str) -> None:
        mode = stat.S_IMODE(path.stat().st_mode)
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{path.name}.norminette-fix.",
            dir=path.parent,
        )
        temporary = Path(temporary_name)
        try:
            with os.fdopen(descriptor, "w", encoding="utf-8", newline="") as handle:
                handle.write(source)
                handle.flush()
                os.fsync(handle.fileno())
            shutil.copystat(path, temporary, follow_symlinks=False)
            os.chmod(temporary, mode)
            os.replace(temporary, path)
            directory_descriptor = os.open(path.parent, os.O_RDONLY)
            try:
                os.fsync(directory_descriptor)
            finally:
                os.close(directory_descriptor)
        finally:
            if temporary.exists():
                temporary.unlink()
