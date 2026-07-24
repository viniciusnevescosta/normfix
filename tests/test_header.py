from datetime import datetime
from pathlib import Path

from norminette_fix.header import (
    HEADER_EDGE,
    build_header,
    ensure_header,
    ensure_header_guard,
    expected_guard,
    header_filename_matches,
    resolve_identity,
    update_header,
)
from norminette_fix.models import Identity
from norminette_fix.norminette_adapter import NorminetteAdapter

IDENTITY = Identity("vncosta", "vncosta@student.42sp.org", "test")


def test_official_header_shape_and_width() -> None:
    header = build_header(
        "main.c",
        IDENTITY,
        datetime(2026, 7, 23, 12, 34, 56),
    )
    lines = header.splitlines()
    assert len(lines) == 11
    assert all(len(line) == 80 for line in lines)
    assert lines[0] == HEADER_EDGE
    assert lines[-1] == HEADER_EDGE
    assert "main.c" in lines[3]
    assert "By: vncosta <vncosta@student.42sp.org>" in lines[5]
    assert "Created: 2026/07/23 12:34:56 by vncosta" in lines[7]
    assert "Updated: 2026/07/23 12:34:56 by vncosta" in lines[8]


def test_header_is_inserted_once() -> None:
    source = "int\tmain(void)\n{\n\treturn (0);\n}\n"
    first, changed, inserted = ensure_header(source, "main.c", IDENTITY)
    second, changed_again, inserted_again = ensure_header(first, "main.c", IDENTITY)
    assert changed and inserted
    assert not changed_again and not inserted_again
    assert second == first


def test_long_identity_is_not_truncated_into_a_header() -> None:
    identity = Identity(
        "verylongstudentlogin",
        "verylongstudentlogin@student.42campus.example",
        "test",
    )
    source = "int\tmain(void)\n{\n\treturn (0);\n}\n"

    fixed, changed, inserted = ensure_header(source, "main.c", identity)

    assert not changed and not inserted
    assert fixed == source


def test_stale_header_filename_is_updated_once() -> None:
    source = build_header(
        "old_name.c",
        IDENTITY,
        datetime(2026, 7, 23, 12, 34, 56),
    )
    source += "\n\nint\tmain(void)\n{\n\treturn (0);\n}\n"
    assert not header_filename_matches(source, "main.c")

    updated, changed = update_header(
        source,
        "main.c",
        IDENTITY,
        datetime(2026, 7, 23, 13, 0, 0),
    )

    assert changed
    assert header_filename_matches(updated, "main.c")


def test_generated_header_passes_official_norminette() -> None:
    header = build_header(
        "main.c",
        IDENTITY,
        datetime(2026, 7, 23, 12, 34, 56),
    )
    source = header + "\n\nint\tmain(void)\n{\n\treturn (0);\n}\n"
    diagnostics, failure = NorminetteAdapter().lint(Path("main.c"), source)
    assert failure is None
    assert diagnostics == []


def test_header_guard_is_derived_from_filename() -> None:
    header = build_header("ft_demo.h", IDENTITY)
    source, changed, guard = ensure_header_guard(
        header + "\n\nint\tft_demo(void);\n",
        "ft_demo.h",
    )
    assert not changed
    assert guard == expected_guard("ft_demo.h") == "FT_DEMO_H"
    assert "#ifndef FT_DEMO_H" not in source


def test_existing_guard_allows_comments_before_and_after_it() -> None:
    body = (
        "// public API\n"
        "#ifndef FT_DEMO_H\n"
        "# define FT_DEMO_H\n"
        "\n"
        "int\tft_demo(void);\n"
        "\n"
        "#endif\n"
        "// end of public API\n"
    )

    source, changed, guard = ensure_header_guard(body, "ft_demo.h")

    assert guard == "FT_DEMO_H"
    assert not changed
    assert source == body
    assert source.count("#ifndef FT_DEMO_H") == 1


def test_partial_same_name_guard_is_not_nested() -> None:
    body = (
        "#ifndef FT_DEMO_H\n# define FT_DEMO_H\nint\tinside(void);\n#endif\nint\toutside(void);\n"
    )

    source, changed, guard = ensure_header_guard(body, "ft_demo.h")

    assert guard == "FT_DEMO_H"
    assert not changed
    assert source == body
    assert source.count("#ifndef FT_DEMO_H") == 1


def test_identity_can_come_from_user_config(tmp_path: Path, monkeypatch) -> None:
    config = tmp_path / "config.ini"
    config.write_text(
        "[header]\nlogin = student-a\nemail = student-a@student.42.fr\n",
        encoding="utf-8",
    )
    monkeypatch.setenv("NORMINETTE_FIX_CONFIG", str(config))

    identity = resolve_identity(login=None, email=None, cwd=tmp_path)

    assert identity.login == "student-a"
    assert identity.email == "student-a@student.42.fr"
    assert not identity.inferred


def test_cli_email_ignores_lower_priority_login_settings(
    tmp_path: Path,
    monkeypatch,
) -> None:
    config = tmp_path / "config.ini"
    config.write_text(
        "[header]\nlogin = old-login\nemail = old-login@student.42.fr\n",
        encoding="utf-8",
    )
    monkeypatch.setenv("NORMINETTE_FIX_CONFIG", str(config))
    monkeypatch.setenv("NORMINETTE_FIX_LOGIN", "environment-login")

    identity = resolve_identity(
        login=None,
        email="current-login@student.42.fr",
        cwd=tmp_path,
    )

    assert identity.available
    assert identity.login == "current-login"
    assert identity.email == "current-login@student.42.fr"


def test_malformed_config_value_becomes_unavailable_instead_of_crashing(
    tmp_path: Path,
    monkeypatch,
) -> None:
    config = tmp_path / "config.ini"
    config.write_text(
        "[header]\nemail = bad%email@student.42.fr\n",
        encoding="utf-8",
    )
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("NORMINETTE_FIX_CONFIG", str(config))
    monkeypatch.delenv("NORMINETTE_FIX_EMAIL", raising=False)
    monkeypatch.delenv("NORMINETTE_FIX_LOGIN", raising=False)
    monkeypatch.delenv("MAIL", raising=False)

    identity = resolve_identity(login=None, email=None, cwd=tmp_path)

    assert not identity.available
    assert "does not contain a valid 42 student email" in identity.source


def test_identity_can_come_from_42header_editor_settings(
    tmp_path: Path,
    monkeypatch,
) -> None:
    settings = tmp_path / "Library" / "Application Support" / "Code" / "User" / "settings.json"
    settings.parent.mkdir(parents=True)
    settings.write_text(
        '{"42header.email": "student-a@student.42berlin.de"}\n',
        encoding="utf-8",
    )
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv(
        "NORMINETTE_FIX_CONFIG",
        str(tmp_path / "missing-config.ini"),
    )
    monkeypatch.delenv("NORMINETTE_FIX_EMAIL", raising=False)
    monkeypatch.delenv("NORMINETTE_FIX_LOGIN", raising=False)
    monkeypatch.delenv("MAIL", raising=False)

    identity = resolve_identity(login=None, email=None, cwd=tmp_path)

    assert identity.login == "student-a"
    assert identity.email == "student-a@student.42berlin.de"
    assert identity.available


def test_missing_42_email_warns_by_withholding_header(
    tmp_path: Path,
    monkeypatch,
) -> None:
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv(
        "NORMINETTE_FIX_CONFIG",
        str(tmp_path / "missing-config.ini"),
    )
    monkeypatch.delenv("NORMINETTE_FIX_EMAIL", raising=False)
    monkeypatch.delenv("NORMINETTE_FIX_LOGIN", raising=False)
    monkeypatch.delenv("MAIL", raising=False)
    identity = resolve_identity(login=None, email=None, cwd=tmp_path)
    source = "int\tmain(void)\n{\n\treturn (0);\n}\n"

    fixed, changed, inserted = ensure_header(source, "main.c", identity)

    assert not identity.available
    assert "no 42 student email was found" in identity.source
    assert not changed and not inserted
    assert fixed == source


def test_ambiguous_editor_emails_are_never_guessed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    (tmp_path / ".vimrc").write_text(
        "let g:mail42 = 'first@student.42.fr'\n",
        encoding="utf-8",
    )
    settings = tmp_path / "Library" / "Application Support" / "Code" / "User" / "settings.json"
    settings.parent.mkdir(parents=True)
    settings.write_text(
        '{"42header.email": "second@student.42.fr"}\n',
        encoding="utf-8",
    )
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv(
        "NORMINETTE_FIX_CONFIG",
        str(tmp_path / "missing-config.ini"),
    )
    monkeypatch.delenv("NORMINETTE_FIX_EMAIL", raising=False)
    monkeypatch.delenv("NORMINETTE_FIX_LOGIN", raising=False)
    monkeypatch.delenv("MAIL", raising=False)

    identity = resolve_identity(login=None, email=None, cwd=tmp_path)

    assert not identity.available
    assert "multiple 42 student emails" in identity.source


def test_malformed_header_like_prefix_never_deletes_code() -> None:
    malformed = (
        HEADER_EDGE
        + "\n/* malformed */\n"
        + "int\tkeep_me(void);\n"
        + ("/" + "* filler *" + "/\n") * 7
        + HEADER_EDGE
        + "\n"
    )

    fixed, changed, inserted = ensure_header(
        malformed,
        "safe.h",
        IDENTITY,
    )

    assert changed and inserted
    assert "int\tkeep_me(void);" in fixed
    assert fixed.count(HEADER_EDGE) == 4
