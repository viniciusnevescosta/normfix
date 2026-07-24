import json
from pathlib import Path

from norminette_fix.cli import main

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
    h_file.write_text("int demo(void);\n", encoding="utf-8")
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
    second.write_text("int second(void);\n", encoding="utf-8")
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
    assert payload["files"][0]["changed"] is True
    assert payload["summary"]["files"] == 1
