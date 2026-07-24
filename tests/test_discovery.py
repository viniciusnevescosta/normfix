import subprocess
from pathlib import Path

from pytest import MonkeyPatch

import norminette_fix.discovery as discovery_module
from norminette_fix.discovery import discover, discover_with_warnings


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


def test_makefile_is_processed_as_a_directory_or_explicit_target(tmp_path: Path) -> None:
    makefile = tmp_path / "Makefile"
    makefile.write_text("all:\n", encoding="utf-8")

    directory_result = discover_with_warnings([], cwd=tmp_path)
    explicit_result = discover_with_warnings([str(makefile)], cwd=tmp_path)

    assert directory_result.paths == [makefile]
    assert explicit_result.paths == [makefile]
    assert directory_result.unexpected_files == []
    assert explicit_result.failures == []


def test_directory_scan_reports_only_unexpected_project_files(tmp_path: Path) -> None:
    (tmp_path / "main.c").write_text("", encoding="utf-8")
    (tmp_path / "project.h").write_text("", encoding="utf-8")
    (tmp_path / "Makefile").write_text("", encoding="utf-8")
    (tmp_path / "README.md").write_text("", encoding="utf-8")
    object_file = tmp_path / "main.o"
    notes = tmp_path / "notes.txt"
    uppercase_source = tmp_path / "legacy.C"
    object_file.write_bytes(b"\x00")
    notes.write_text("private notes\n", encoding="utf-8")
    uppercase_source.write_text("", encoding="utf-8")

    result = discover_with_warnings([], cwd=tmp_path)

    assert [path.name for path in result.paths] == ["Makefile", "main.c", "project.h"]
    assert result.failures == []
    assert result.unexpected_files == sorted(
        [object_file, notes, uppercase_source],
        key=str,
    )


def test_explicit_source_does_not_scan_unrequested_siblings_for_warnings(
    tmp_path: Path,
) -> None:
    source = tmp_path / "main.c"
    source.write_text("", encoding="utf-8")
    (tmp_path / "program").write_bytes(b"\x00")

    result = discover_with_warnings([str(source)], cwd=tmp_path)

    assert result.paths == [source]
    assert result.unexpected_files == []


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


def test_gitignore_is_checked_once_for_files_across_one_repository(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    (tmp_path / "src").mkdir()
    (tmp_path / "include").mkdir()
    kept = tmp_path / "src" / "keep.c"
    ignored = tmp_path / "include" / "ignored\nname.h"
    kept.write_text("", encoding="utf-8")
    ignored.write_text("", encoding="utf-8")
    (tmp_path / ".gitignore").write_text("ignored*\n", encoding="utf-8")
    real_run = subprocess.run
    calls: list[tuple[list[str], dict[str, object]]] = []

    def recording_run(
        command: list[str],
        **kwargs: object,
    ) -> subprocess.CompletedProcess[str] | subprocess.CompletedProcess[bytes]:
        calls.append((command, kwargs))
        return real_run(command, **kwargs)

    monkeypatch.setattr(discovery_module.subprocess, "run", recording_run)

    paths, failures = discover([], cwd=tmp_path, use_gitignore=True)

    assert paths == [kept]
    assert failures == []
    rev_parse_calls = [call for call in calls if "rev-parse" in call[0]]
    check_ignore_calls = [call for call in calls if "check-ignore" in call[0]]
    assert len(rev_parse_calls) == 1
    assert len(check_ignore_calls) == 1
    assert check_ignore_calls[0][0][-2:] == ["--stdin", "-z"]
    discovered_order = sorted([kept, ignored], key=str)
    assert check_ignore_calls[0][1]["input"] == b"".join(
        bytes(path) + b"\0" for path in discovered_order
    )


def test_gitignore_batch_failure_falls_back_without_changing_results(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    paths = [tmp_path / "first.c", tmp_path / "second.c", tmp_path / "third.c"]

    def fake_run(
        command: list[str],
        **_kwargs: object,
    ) -> subprocess.CompletedProcess[str] | subprocess.CompletedProcess[bytes]:
        if "rev-parse" in command:
            return subprocess.CompletedProcess(command, 0, stdout=f"{tmp_path}\n")
        if "--stdin" in command:
            return subprocess.CompletedProcess(command, 128, stdout=b"")
        return_codes = {
            str(paths[0]): 1,
            str(paths[1]): 2,
            str(paths[2]): 0,
        }
        return subprocess.CompletedProcess(command, return_codes[command[-1]])

    monkeypatch.setattr(discovery_module.subprocess, "run", fake_run)

    included, failures = discovery_module._remove_gitignored(paths)

    assert included == paths[:2]
    assert failures == [f"Git could not check ignore rules for '{paths[1]}'."]
