from __future__ import annotations

import getpass
import os
import re
import subprocess
from datetime import datetime
from pathlib import Path

from .models import Identity

HEADER_EDGE = "/* " + ("*" * 74) + " */"
HEADER_SUFFIXES = {
    "file": ":+:      :+:    :+:   ",
    "by": "+#+  +:+       +#+        ",
    "created": "#+#    #+#             ",
    "updated": "###   ########.fr       ",
}


def _git_config(key: str, cwd: Path) -> str | None:
    try:
        result = subprocess.run(
            ["git", "config", "--get", key],
            cwd=cwd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    except OSError:
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
    git_name = _git_config("user.name", cwd)
    git_email = _git_config("user.email", cwd)

    inferred_login = login is None and env_login is None
    inferred_email = email is None and env_email is None
    chosen_login = login or env_login
    source_parts: list[str] = []
    if login or email:
        source_parts.append("command line")
    if env_login or env_email:
        source_parts.append("environment")
    if chosen_login is None and git_name:
        compact = re.sub(r"[^A-Za-z0-9_-]", "", git_name)
        chosen_login = compact or None
        source_parts.append("Git config")
    if chosen_login is None:
        chosen_login = getpass.getuser()
        source_parts.append("system user")

    chosen_email = email or env_email
    if (
        chosen_email is None
        and git_email
        and ("@student." in git_email or git_email.endswith("@42.fr"))
    ):
        chosen_email = git_email
        source_parts.append("Git config")
    if chosen_email is None:
        chosen_email = f"{chosen_login}@student.42sp.org"
        source_parts.append("derived student address")

    return Identity(
        login=chosen_login,
        email=chosen_email,
        source=", ".join(dict.fromkeys(source_parts)) or "defaults",
        inferred_login=inferred_login,
        inferred_email=inferred_email,
    )


def _framed(left: str = "", right: str = "") -> str:
    available = 76 - len(right)
    if len(left) > available:
        left = left[: max(0, available - 3)] + "..."
    return "/*" + left + (" " * (76 - len(left) - len(right))) + right + "*/"


def build_header(filename: str, identity: Identity, now: datetime | None = None) -> str:
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
    # Never delete a malformed "header-like" prefix: it may contain real code.
    header = build_header(filename, identity)
    body = source.lstrip("\n")
    return header + "\n\n" + body, True, True


def update_header(
    source: str,
    filename: str,
    identity: Identity,
    now: datetime | None = None,
) -> tuple[str, bool]:
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


def ensure_header_guard(source: str, filename: str) -> tuple[str, bool, str]:
    guard = expected_guard(filename)
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", guard):
        return source, False, guard
    span = header_span(source)
    body_start = span[1] if span else 0
    body = source[body_start:].lstrip("\n")

    canonical = _canonical_guard(body)
    if canonical is not None:
        ifndef, define = canonical
        changed = False
        for match in sorted((ifndef, define), key=lambda item: item.start(1), reverse=True):
            if match.group(1) != guard:
                body = body[: match.start(1)] + guard + body[match.end(1) :]
                changed = True
        if not changed:
            return source, False, guard
        prefix = source[:body_start].rstrip("\n") + "\n\n"
        return prefix + body.lstrip("\n"), True, guard

    wrapped = f"#ifndef {guard}\n# define {guard}\n\n"
    wrapped += body.rstrip("\n")
    if body.strip():
        wrapped += "\n\n"
    wrapped += "#endif\n"
    prefix = source[:body_start].rstrip("\n") + "\n\n"
    return prefix + wrapped, True, guard


def _canonical_guard(body: str) -> tuple[re.Match[str], re.Match[str]] | None:
    lines = body.splitlines()
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
        or re.fullmatch(r"#\s*endif(?:\s*/\*.*\*/\s*)?", lines[last]) is None
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

    ifndef = re.search(
        r"(?m)^#\s*ifndef\s+([A-Za-z_][A-Za-z0-9_]*)\s*$",
        body,
    )
    define = re.search(
        r"(?m)^#\s*define\s+([A-Za-z_][A-Za-z0-9_]*)\s*$",
        body,
    )
    if ifndef is None or define is None:
        return None
    return ifndef, define
