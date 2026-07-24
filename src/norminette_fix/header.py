from __future__ import annotations

import configparser
import hashlib
import os
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from .models import Identity
from .source import masked_source, normalize_hygiene

HEADER_EDGE = "/* " + ("*" * 74) + " */"
HEADER_SUFFIXES = {
    "file": ":+:      :+:    :+:   ",
    "by": "+#+  +:+       +#+        ",
    "created": "#+#    #+#             ",
    "updated": "###   ########.fr       ",
}


@dataclass(frozen=True)
class HeaderGuardRename:
    current: str
    expected: str
    body_sha256: str


def _git_config(key: str, cwd: Path) -> str | None:
    try:
        result = subprocess.run(
            ["git", "config", "--get", key],
            cwd=cwd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
            timeout=2,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    value = result.stdout.strip()
    return value or None


def resolve_identity(
    *,
    login: str | None,
    email: str | None,
    cwd: Path,
) -> Identity:
    env_login = os.environ.get("NORMINETTE_FIX_LOGIN")
    env_email = os.environ.get("NORMINETTE_FIX_EMAIL")
    config_login, config_email = _configured_identity()
    git_email = _git_config("user.email", cwd)

    explicit_emails = (
        (email, login, "command line"),
        (env_email, login or env_login, "environment"),
        (config_email, login or env_login or config_login, "user config"),
    )
    for configured_email, matching_login, source in explicit_emails:
        if configured_email is None:
            continue
        return _identity_from_email(
            configured_email,
            requested_login=matching_login,
            source=source,
            inferred=False,
        )

    requested_login = login or env_login or config_login
    rejected_sources: list[str] = []
    if git_email and _canonical_42_email(git_email):
        git_identity = _identity_from_email(
            git_email,
            requested_login=requested_login,
            source="Git config",
            inferred=True,
        )
        if git_identity.available:
            return git_identity
        rejected_sources.append(git_identity.source)

    mail_environment = os.environ.get("MAIL")
    if mail_environment and _canonical_42_email(mail_environment):
        mail_identity = _identity_from_email(
            mail_environment,
            requested_login=requested_login,
            source="MAIL environment variable",
            inferred=True,
        )
        if mail_identity.available:
            return mail_identity
        rejected_sources.append(mail_identity.source)

    candidates = _saved_editor_emails()
    selected = _select_saved_email(candidates, requested_login=requested_login)
    if selected is None:
        if candidates:
            reason = (
                "saved editor settings contain multiple 42 student emails, but "
                "none could be matched safely to the configured login"
            )
        elif rejected_sources:
            reason = "; ".join(rejected_sources)
        else:
            reason = (
                "no 42 student email was found in the command settings, "
                "environment, Git, Vim/Neovim, or VS Code/Cursor settings"
            )
        return Identity(
            login="",
            email="",
            source=reason,
            inferred_login=True,
            inferred_email=True,
        )

    sources = ", ".join(sorted(candidates[selected]))
    return _identity_from_email(
        selected,
        requested_login=requested_login,
        source=sources,
        inferred=True,
    )


def _identity_from_email(
    email: str,
    *,
    requested_login: str | None,
    source: str,
    inferred: bool,
) -> Identity:
    canonical = _canonical_42_email(email)
    if canonical is None:
        return Identity(
            login="",
            email="",
            source=f"{source} does not contain a valid 42 student email",
            inferred_login=True,
            inferred_email=True,
        )
    email_login = canonical.split("@", 1)[0]
    if requested_login and requested_login.casefold() != email_login.casefold():
        return Identity(
            login="",
            email="",
            source=(
                f"{source} contains {canonical}, which does not match the "
                f"configured login {requested_login}"
            ),
            inferred_login=True,
            inferred_email=True,
        )
    return Identity(
        login=email_login,
        email=canonical,
        source=source,
        inferred_login=inferred and requested_login is None,
        inferred_email=inferred,
    )


def identity_from_email(
    email: str,
    *,
    login: str | None = None,
    source: str = "interactive terminal",
) -> Identity:
    """Validate a stored/entered 42 email and derive its matching login."""
    return _identity_from_email(
        email,
        requested_login=login,
        source=source,
        inferred=False,
    )


def _canonical_42_email(value: str) -> str | None:
    match = re.fullmatch(
        r"([A-Za-z0-9][A-Za-z0-9._-]*)@"
        r"(42\.fr|student\.42[A-Za-z0-9-]*(?:\.[A-Za-z0-9-]+)+)",
        value.strip(),
        re.IGNORECASE,
    )
    if match is None:
        return None
    return f"{match.group(1)}@{match.group(2)}".lower()


def _saved_editor_emails() -> dict[str, set[str]]:
    home = Path.home()
    candidates: dict[str, set[str]] = {}
    locations = (
        (
            home / ".vimrc",
            r"\bg:mail42\s*=\s*['\"]([^'\"]+)['\"]",
            "Vim settings",
        ),
        (
            home / ".config" / "nvim" / "init.vim",
            r"\bg:mail42\s*=\s*['\"]([^'\"]+)['\"]",
            "Neovim settings",
        ),
        (
            home / ".config" / "nvim" / "init.lua",
            r"\bvim\.g\.mail42\s*=\s*['\"]([^'\"]+)['\"]",
            "Neovim settings",
        ),
        (
            home / ".zshrc",
            r"(?m)^[ \t]*(?:export[ \t]+)?MAIL\s*=\s*['\"]?([^'\"\s#]+)",
            "shell settings",
        ),
        (
            home / ".zprofile",
            r"(?m)^[ \t]*(?:export[ \t]+)?MAIL\s*=\s*['\"]?([^'\"\s#]+)",
            "shell settings",
        ),
        (
            home / ".bashrc",
            r"(?m)^[ \t]*(?:export[ \t]+)?MAIL\s*=\s*['\"]?([^'\"\s#]+)",
            "shell settings",
        ),
        (
            home / ".bash_profile",
            r"(?m)^[ \t]*(?:export[ \t]+)?MAIL\s*=\s*['\"]?([^'\"\s#]+)",
            "shell settings",
        ),
        (
            home / "Library" / "Application Support" / "Code" / "User" / "settings.json",
            r"\"42header\.email\"\s*:\s*\"([^\"]+)\"",
            "VS Code settings",
        ),
        (
            home / "Library" / "Application Support" / "Cursor" / "User" / "settings.json",
            r"\"42header\.email\"\s*:\s*\"([^\"]+)\"",
            "Cursor settings",
        ),
        (
            home / ".config" / "Code" / "User" / "settings.json",
            r"\"42header\.email\"\s*:\s*\"([^\"]+)\"",
            "VS Code settings",
        ),
        (
            home / ".config" / "VSCodium" / "User" / "settings.json",
            r"\"42header\.email\"\s*:\s*\"([^\"]+)\"",
            "VSCodium settings",
        ),
        (
            home / ".config" / "Cursor" / "User" / "settings.json",
            r"\"42header\.email\"\s*:\s*\"([^\"]+)\"",
            "Cursor settings",
        ),
    )
    for path, pattern, source in locations:
        try:
            if not path.is_file() or path.stat().st_size > 1_000_000:
                continue
            content = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        for match in re.finditer(pattern, content):
            canonical = _canonical_42_email(match.group(1))
            if canonical is None:
                continue
            candidates.setdefault(canonical, set()).add(source)
    return candidates


def _select_saved_email(
    candidates: dict[str, set[str]],
    *,
    requested_login: str | None,
) -> str | None:
    if not candidates:
        return None
    if requested_login:
        matches = [
            email
            for email in candidates
            if email.split("@", 1)[0].casefold() == requested_login.casefold()
        ]
        return matches[0] if len(matches) == 1 else None
    if len(candidates) == 1:
        return next(iter(candidates))
    return None


def _configured_identity() -> tuple[str | None, str | None]:
    configured_path = os.environ.get("NORMINETTE_FIX_CONFIG")
    if configured_path:
        path = Path(configured_path).expanduser()
    else:
        config_home = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
        path = config_home / "norminette-fix" / "config.ini"
    parser = configparser.ConfigParser(interpolation=None)
    try:
        with path.open(encoding="utf-8") as handle:
            parser.read_file(handle)
        if not parser.has_section("header"):
            return None, None
        login = parser.get("header", "login", fallback="").strip() or None
        email = parser.get("header", "email", fallback="").strip() or None
    except (OSError, UnicodeError, configparser.Error):
        return None, None
    return login, email


def identity_fits_header(identity: Identity) -> bool:
    if not identity.available:
        return True
    timestamp = "0000/00/00 00:00:00"
    fields = (
        (
            f"   By: {identity.login} <{identity.email}>",
            HEADER_SUFFIXES["by"],
        ),
        (
            f"   Created: {timestamp} by {identity.login}",
            HEADER_SUFFIXES["created"],
        ),
        (
            f"   Updated: {timestamp} by {identity.login}",
            HEADER_SUFFIXES["updated"],
        ),
    )
    return all(len(left) <= 76 - len(right) for left, right in fields)


def _framed(left: str = "", right: str = "") -> str:
    available = 76 - len(right)
    if len(left) > available:
        left = left[: max(0, available - 3)] + "..."
    return "/*" + left + (" " * (76 - len(left) - len(right))) + right + "*/"


def build_header(filename: str, identity: Identity, now: datetime | None = None) -> str:
    if not identity.available:
        raise ValueError("A verified 42 student email is required for the official header.")
    if not identity_fits_header(identity):
        raise ValueError("The verified 42 identity does not fit the official 80-column header.")
    now = now or datetime.now()
    timestamp = now.strftime("%Y/%m/%d %H:%M:%S")
    return "\n".join(
        (
            HEADER_EDGE,
            _framed(),
            _framed("", ":::      ::::::::   "),
            _framed(f"   {filename}", HEADER_SUFFIXES["file"]),
            _framed("", "+:+ +:+         +:+     "),
            _framed(
                f"   By: {identity.login} <{identity.email}>",
                HEADER_SUFFIXES["by"],
            ),
            _framed("", ("+#" * 5) + "+   +#+           "),
            _framed(
                f"   Created: {timestamp} by {identity.login}",
                HEADER_SUFFIXES["created"],
            ),
            _framed(
                f"   Updated: {timestamp} by {identity.login}",
                HEADER_SUFFIXES["updated"],
            ),
            _framed(),
            HEADER_EDGE,
        )
    )


def header_span(source: str) -> tuple[int, int] | None:
    lines = source.splitlines(keepends=True)
    if len(lines) < 11:
        return None
    if lines[0].rstrip("\r\n") != HEADER_EDGE:
        return None
    if lines[10].rstrip("\r\n") != HEADER_EDGE:
        return None
    return 0, sum(len(line) for line in lines[:11])


def _valid_header_block(block: str) -> bool:
    lines = block.rstrip("\r\n").splitlines()
    if len(lines) != 11 or any(len(line) != 80 for line in lines):
        return False
    if lines[0] != HEADER_EDGE or lines[10] != HEADER_EDGE:
        return False
    if lines[1] != _framed() or lines[9] != _framed():
        return False
    if lines[2] != _framed("", ":::      ::::::::   "):
        return False
    if lines[4] != _framed("", "+:+ +:+         +:+     "):
        return False
    if lines[6] != _framed("", ("+#" * 5) + "+   +#+           "):
        return False
    variable_lines = (
        (
            lines[3],
            r"^/\*   \S+",
            HEADER_SUFFIXES["file"],
        ),
        (
            lines[5],
            r"^/\*   By: \S+ <[^<> ]+>",
            HEADER_SUFFIXES["by"],
        ),
        (
            lines[7],
            r"^/\*   Created: \d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} by \S+",
            HEADER_SUFFIXES["created"],
        ),
        (
            lines[8],
            r"^/\*   Updated: \d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} by \S+",
            HEADER_SUFFIXES["updated"],
        ),
    )
    return all(
        re.match(pattern, line) and line.endswith(suffix + "*/")
        for line, pattern, suffix in variable_lines
    )


def ensure_header(
    source: str,
    filename: str,
    identity: Identity,
) -> tuple[str, bool, bool]:
    """Return source, changed, inserted."""
    span = header_span(source)
    if span is not None and _valid_header_block(source[span[0] : span[1]]):
        return source, False, False
    if not identity.available or not identity_fits_header(identity):
        return source, False, False
    # Never delete a malformed "header-like" prefix: it may contain real code.
    header = build_header(filename, identity)
    return header + "\n\n" + source, True, True


def update_header(
    source: str,
    filename: str,
    identity: Identity,
    now: datetime | None = None,
) -> tuple[str, bool]:
    if not identity.available or not identity_fits_header(identity):
        return source, False
    span = header_span(source)
    if span is None or not _valid_header_block(source[span[0] : span[1]]):
        return source, False
    now = now or datetime.now()
    timestamp = now.strftime("%Y/%m/%d %H:%M:%S")
    block = source[span[0] : span[1]]
    lines = block.rstrip("\n").splitlines()
    if len(lines) < 11:
        return source, False

    new_file_line = _framed(f"   {filename}", HEADER_SUFFIXES["file"])
    updated_line = _framed(
        f"   Updated: {timestamp} by {identity.login}",
        HEADER_SUFFIXES["updated"],
    )
    changed = lines[3] != new_file_line or lines[8] != updated_line
    lines[3] = new_file_line
    lines[8] = updated_line
    replacement = "\n".join(lines) + "\n"
    return source[: span[0]] + replacement + source[span[1] :], changed


def header_filename_matches(source: str, filename: str) -> bool:
    span = header_span(source)
    if span is None or not _valid_header_block(source[span[0] : span[1]]):
        return False
    lines = source[span[0] : span[1]].rstrip("\n").splitlines()
    if len(lines) < 11:
        return False
    return lines[3] == _framed(f"   {filename}", HEADER_SUFFIXES["file"])


def expected_guard(filename: str) -> str:
    return re.sub(r"[^A-Za-z0-9]", "_", filename).upper()


def header_guard_rename_candidate(
    source: str,
    filename: str,
) -> HeaderGuardRename | None:
    source, _ = normalize_hygiene(source)
    guard = expected_guard(filename)
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", guard):
        return None
    _, body = _header_guard_body(source)
    canonical = _canonical_guard(body)
    if canonical is None:
        return None
    ifndef, define = canonical
    if ifndef[2] != define[2] or ifndef[2] == guard:
        return None
    return HeaderGuardRename(ifndef[2], guard, _guard_body_digest(body))


def header_guard_matches(source: str, filename: str) -> bool:
    guard = expected_guard(filename)
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", guard):
        return False
    _, body = _header_guard_body(source)
    canonical = _canonical_guard(body)
    if canonical is None:
        return False
    ifndef, define = canonical
    return ifndef[2] == guard and define[2] == guard


def ensure_header_guard(
    source: str,
    filename: str,
    *,
    approved_rename: HeaderGuardRename | None = None,
) -> tuple[str, bool, str]:
    guard = expected_guard(filename)
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", guard):
        return source, False, guard
    body_start, body = _header_guard_body(source)

    canonical = _canonical_guard(body)
    if canonical is not None:
        ifndef, define = canonical
        if ifndef[2] == guard and define[2] == guard:
            return source, False, guard
        candidate = HeaderGuardRename(ifndef[2], guard, _guard_body_digest(body))
        if approved_rename == candidate:
            for start, end, current in sorted(
                (ifndef, define),
                key=lambda item: item[0],
                reverse=True,
            ):
                if current != candidate.current:
                    return source, False, guard
                body = body[:start] + guard + body[end:]
            return source[:body_start] + body, True, guard
        # Renaming an existing guard can break consumers that test the old
        # macro, or nested conditions that reference it. Report the official
        # diagnostic and leave this project-wide symbol unchanged.
        return source, False, guard

    # Adding any new guard changes a macro visible to consumers and can break
    # intentional repeat-inclusion protocols (including X-macros). There is no
    # file-local proof that wrapping is safe, so missing guards stay manual.
    return source, False, guard


def _header_guard_body(source: str) -> tuple[int, str]:
    span = header_span(source)
    body_start = span[1] if span else 0
    while body_start < len(source) and source[body_start] == "\n":
        body_start += 1
    return body_start, source[body_start:]


def _guard_body_digest(body: str) -> str:
    normalized, _ = normalize_hygiene(body)
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()


def _canonical_guard(
    body: str,
) -> tuple[tuple[int, int, str], tuple[int, int, str]] | None:
    raw_lines = body.splitlines(keepends=True)
    lines = masked_source(body).splitlines()
    nonempty = [index for index, line in enumerate(lines) if line.strip()]
    if len(nonempty) < 3:
        return None
    first, second, last = nonempty[0], nonempty[1], nonempty[-1]
    ifndef_line = re.fullmatch(
        r"#\s*ifndef\s+([A-Za-z_][A-Za-z0-9_]*)\s*",
        lines[first],
    )
    define_line = re.fullmatch(
        r"#\s*define\s+([A-Za-z_][A-Za-z0-9_]*)\s*",
        lines[second],
    )
    if (
        ifndef_line is None
        or define_line is None
        or ifndef_line.group(1) != define_line.group(1)
        or re.fullmatch(r"#\s*endif\s*", lines[last]) is None
    ):
        return None

    depth = 0
    for index in range(first, last + 1):
        directive = re.match(r"#\s*(if|ifdef|ifndef|endif)\b", lines[index])
        if directive is None:
            continue
        if directive.group(1) in {"if", "ifdef", "ifndef"}:
            depth += 1
        else:
            depth -= 1
            if depth == 0 and index != last:
                return None
        if depth < 0:
            return None
    if depth != 0:
        return None

    offsets: list[int] = []
    offset = 0
    for line in raw_lines:
        offsets.append(offset)
        offset += len(line)

    ifndef = _macro_span(body, offsets[first], "ifndef")
    define = _macro_span(body, offsets[second], "define")
    if ifndef is None or define is None:
        return None
    return ifndef, define


def _macro_span(
    body: str,
    line_start: int,
    directive: str,
) -> tuple[int, int, str] | None:
    line_end = body.find("\n", line_start)
    if line_end < 0:
        line_end = len(body)
    match = re.match(
        rf"[ \t]*#[ \t]*{directive}[ \t]+([A-Za-z_][A-Za-z0-9_]*)",
        body[line_start:line_end],
    )
    if match is None:
        return None
    return (
        line_start + match.start(1),
        line_start + match.end(1),
        match.group(1),
    )
