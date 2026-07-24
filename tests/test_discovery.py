from pathlib import Path

from norminette_fix.discovery import discover


def test_default_directory_scan_is_recursive_and_sorted(tmp_path: Path) -> None:
    (tmp_path / "src").mkdir()
    (tmp_path / "include").mkdir()
    (tmp_path / "src" / "b.c").write_text("", encoding="utf-8")
    (tmp_path / "include" / "a.h").write_text("", encoding="utf-8")
    (tmp_path / "README.md").write_text("", encoding="utf-8")

    paths, failures = discover([], cwd=tmp_path)

    assert failures == []
    assert [path.name for path in paths] == ["a.h", "b.c"]


def test_multiple_files_and_directories_are_accepted(tmp_path: Path) -> None:
    directory = tmp_path / "lib"
    directory.mkdir()
    first = tmp_path / "main.c"
    second = directory / "lib.h"
    first.write_text("", encoding="utf-8")
    second.write_text("", encoding="utf-8")

    paths, failures = discover(
        [str(first), str(directory), str(first)],
        cwd=tmp_path,
    )

    assert failures == []
    assert paths == sorted([first, second], key=str)
