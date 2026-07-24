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


def test_explicit_path_through_symlinked_directory_is_refused(tmp_path: Path) -> None:
    outside = tmp_path.parent / f"{tmp_path.name}-outside"
    outside.mkdir()
    victim = outside / "victim.c"
    victim.write_text("int\tvictim(void);\n", encoding="utf-8")
    link = tmp_path / "other_project"
    link.symlink_to(outside, target_is_directory=True)

    paths, failures = discover(
        [str(link / "victim.c")],
        cwd=tmp_path,
    )

    assert paths == []
    assert len(failures) == 1
    assert "passes through symbolic link" in failures[0]
