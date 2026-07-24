import json
import sys
from io import StringIO
from pathlib import Path

from rich.console import Console

from norminette_fix.cli import _prompt_for_identity, main

IDENTITY_ARGS = [
    "--login",
    "vncosta",
    "--email",
    "vncosta@student.42sp.org",
]


def test_default_scan_fixes_every_c_and_h_file(tmp_path: Path, monkeypatch, capsys) -> None:
    (tmp_path / "src").mkdir()
    (tmp_path / "include").mkdir()
    c_file = tmp_path / "src" / "main.c"
    h_file = tmp_path / "include" / "demo.h"
    c_file.write_text("int main(){return 0;}\n", encoding="utf-8")
    h_file.write_text(
        "#ifndef DEMO_H\n#define DEMO_H\n\nint demo(void);\n\n#endif\n",
        encoding="utf-8",
    )
    monkeypatch.chdir(tmp_path)

    exit_code = main([*IDENTITY_ARGS, "--no-backup", "--no-color"])

    assert exit_code == 0
    assert "official 42 header" not in capsys.readouterr().out
    assert c_file.read_text(encoding="utf-8").startswith("/* " + "*" * 74)
    header = h_file.read_text(encoding="utf-8")
    assert header.startswith("/* " + "*" * 74)
    assert "#ifndef DEMO_H" in header


def test_check_and_diff_do_not_write(tmp_path: Path, monkeypatch, capsys) -> None:
    path = tmp_path / "main.c"
    original = "int main(){return 0;}\n"
    path.write_text(original, encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    check_exit = main([*IDENTITY_ARGS, "--check", "--no-color", str(path)])
    check_output = capsys.readouterr().out
    diff_exit = main([*IDENTITY_ARGS, "--diff", "--no-color", str(path)])
    diff_output = capsys.readouterr().out

    assert check_exit == 1
    assert diff_exit == 1
    assert path.read_text(encoding="utf-8") == original
    assert "WOULD FIX" in check_output
    assert "--- a/main.c" in diff_output
    assert "+++ b/main.c" in diff_output
    assert "+/* " + ("*" * 74) + " */" in diff_output
    assert "+\treturn (0);" in diff_output


def test_multiple_explicit_targets_only_touch_requested_files(tmp_path: Path, monkeypatch) -> None:
    first = tmp_path / "first.c"
    second = tmp_path / "second.h"
    untouched = tmp_path / "untouched.c"
    first.write_text("int first(){return 1;}\n", encoding="utf-8")
    second.write_text(
        "#ifndef SECOND_H\n#define SECOND_H\n\nint second(void);\n\n#endif\n",
        encoding="utf-8",
    )
    untouched.write_text("int untouched(){return 0;}\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    exit_code = main(
        [
            *IDENTITY_ARGS,
            "--no-backup",
            "--no-color",
            str(first),
            str(second),
        ]
    )

    assert exit_code == 0
    assert first.read_text(encoding="utf-8").startswith("/* " + "*" * 74)
    assert second.read_text(encoding="utf-8").startswith("/* " + "*" * 74)
    assert untouched.read_text(encoding="utf-8") == "int untouched(){return 0;}\n"


def test_json_output_is_machine_readable(tmp_path: Path, monkeypatch, capsys) -> None:
    path = tmp_path / "main.c"
    path.write_text("int main(){return 0;}\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    exit_code = main([*IDENTITY_ARGS, "--check", "--format", "json", str(path)])
    payload = json.loads(capsys.readouterr().out)

    assert exit_code == 1
    assert payload["mode"] == "check"
    assert payload["identity"]["available"] is True
    assert payload["files"][0]["changed"] is True
    assert payload["summary"]["files"] == 1
    assert payload["duration_seconds"] >= 0


def test_missing_42_email_warns_and_does_not_invent_header(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    path = tmp_path / "main.c"
    path.write_text("int main(){return 0;}\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv(
        "NORMINETTE_FIX_CONFIG",
        str(tmp_path / "missing-config.ini"),
    )
    monkeypatch.delenv("NORMINETTE_FIX_EMAIL", raising=False)
    monkeypatch.delenv("NORMINETTE_FIX_LOGIN", raising=False)
    monkeypatch.delenv("MAIL", raising=False)

    exit_code = main(["--no-backup", "--no-color", str(path)])
    output = capsys.readouterr().out
    fixed = path.read_text(encoding="utf-8")

    assert exit_code == 1
    assert "Official header not added" in output
    assert not fixed.startswith("/* " + "*" * 74)
    assert "int\tmain(void)" in fixed


def test_interactive_email_prompt_retries_then_accepts() -> None:
    answers = iter(("not-an-email", "student-a@student.42.fr"))
    output = StringIO()

    identity = _prompt_for_identity(
        requested_login=None,
        reader=lambda: next(answers),
        console=Console(file=output, no_color=True, force_terminal=False),
    )

    assert identity.login == "student-a"
    assert identity.email == "student-a@student.42.fr"
    assert "Invalid identity" in output.getvalue()
    assert "Using student-a" in output.getvalue()


def test_interactive_email_prompt_can_be_cancelled() -> None:
    output = StringIO()

    identity = _prompt_for_identity(
        requested_login=None,
        reader=lambda: "",
        console=Console(file=output, no_color=True, force_terminal=False),
    )

    assert not identity.available
    assert "cancelled" in identity.source
    assert "cancelled" in output.getvalue()


def test_hostile_identity_text_cannot_break_pretty_report(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    path = tmp_path / "main.c"
    path.write_text("int main(){return 0;}\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    exit_code = main(
        [
            "--login",
            "[/dim]",
            "--email",
            "student-a@student.42.fr",
            "--no-backup",
            "--no-color",
            str(path),
        ]
    )
    output = capsys.readouterr().out

    assert exit_code == 1
    assert "Official header not added" in output
    assert "does not match" in output


def test_main_prompts_once_when_terminal_has_no_saved_email(
    tmp_path: Path,
    monkeypatch,
) -> None:
    class TerminalInput(StringIO):
        def isatty(self) -> bool:
            return True

    path = tmp_path / "main.c"
    path.write_text("int main(){return 0;}\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv(
        "NORMINETTE_FIX_CONFIG",
        str(tmp_path / "missing-config.ini"),
    )
    monkeypatch.delenv("NORMINETTE_FIX_EMAIL", raising=False)
    monkeypatch.delenv("NORMINETTE_FIX_LOGIN", raising=False)
    monkeypatch.delenv("MAIL", raising=False)
    monkeypatch.setattr(
        sys,
        "stdin",
        TerminalInput("student-a@student.42.fr\n"),
    )

    exit_code = main(["--no-backup", "--no-color", str(path)])
    fixed = path.read_text(encoding="utf-8")

    assert exit_code == 0
    assert "By: student-a <student-a@student.42.fr>" in fixed


def test_unexpected_project_files_are_warnings_only(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    source = tmp_path / "main.c"
    source.write_text("int main(){return 0;}\n", encoding="utf-8")
    (tmp_path / "Makefile").write_text(
        "NAME = demo\n"
        "SRC = main.c\n"
        "OBJ = $(SRC:.c=.o)\n"
        "all: $(NAME)\n"
        "$(NAME): $(OBJ)\n"
        "\t$(CC) $(OBJ) -o $(NAME)\n"
        "clean:\n"
        "\trm -f $(OBJ)\n"
        "fclean: clean\n"
        "\trm -f $(NAME)\n"
        "re: fclean all\n"
        ".PHONY: all clean fclean re\n",
        encoding="utf-8",
    )
    (tmp_path / "README.md").write_text("# Demo\n", encoding="utf-8")
    executable = tmp_path / "demo"
    executable.write_bytes(b"binary")
    monkeypatch.chdir(tmp_path)

    exit_code = main([*IDENTITY_ARGS, "--no-backup", "--no-color"])
    output = capsys.readouterr().out

    assert exit_code == 0
    assert "Unexpected project files (not modified)" in output
    assert "demo" in output
    assert executable.read_bytes() == b"binary"
    assert "Completed in " in output


def test_invalid_comments_are_reported_by_default_and_removed_only_with_flag(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    source = tmp_path / "main.c"
    source.write_text(
        "int\tmain(void)\n{\n\t/* remove me */\n\treturn (0);\n}\n",
        encoding="utf-8",
    )
    monkeypatch.chdir(tmp_path)

    first_exit = main([*IDENTITY_ARGS, "--no-backup", "--no-color", str(source)])
    first_output = capsys.readouterr().out
    first_source = source.read_text(encoding="utf-8")

    assert first_exit == 1
    assert "WRONG_SCOPE_COMMENT" in first_output
    assert "--remove-invalid-comments" in first_output
    assert "/* remove me */" in first_source

    second_exit = main(
        [
            *IDENTITY_ARGS,
            "--no-backup",
            "--no-color",
            "--remove-invalid-comments",
            str(source),
        ]
    )
    second_output = capsys.readouterr().out
    second_source = source.read_text(encoding="utf-8")

    assert second_exit == 0
    assert "/* remove me */" not in second_source
    assert second_source.startswith("/* " + ("*" * 74) + " */")
    assert "REMOVE_INVALID_COMMENT" not in second_output


def test_comment_removal_flag_keeps_allowed_global_comments(
    tmp_path: Path,
    monkeypatch,
) -> None:
    source = tmp_path / "main.c"
    source.write_text(
        "/* This global comment is allowed. */\nint\tmain(void)\n{\n\treturn (0);\n}\n",
        encoding="utf-8",
    )
    monkeypatch.chdir(tmp_path)

    exit_code = main(
        [
            *IDENTITY_ARGS,
            "--no-backup",
            "--no-color",
            "--remove-invalid-comments",
            str(source),
        ]
    )

    assert exit_code == 0
    assert "/* This global comment is allowed. */" in source.read_text(encoding="utf-8")


def test_json_reports_unexpected_files_and_total_duration(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    source = tmp_path / "main.c"
    source.write_text("int\tmain(void)\n{\n\treturn (0);\n}\n", encoding="utf-8")
    unexpected = tmp_path / "program.out"
    unexpected.write_bytes(b"binary")
    monkeypatch.chdir(tmp_path)

    exit_code = main(
        [
            *IDENTITY_ARGS,
            "--check",
            "--format",
            "json",
        ]
    )
    payload = json.loads(capsys.readouterr().out)

    assert exit_code == 1
    assert payload["unexpected_files"] == [str(unexpected)]
    assert payload["summary"]["unexpected_files"] == 1
    assert isinstance(payload["duration_seconds"], float)
    assert payload["duration_seconds"] >= 0
