import subprocess
from pathlib import Path

import pytest
from pytest import MonkeyPatch

import norminette_fix.engine as engine_module
from norminette_fix.engine import EngineOptions, FixEngine
from norminette_fix.guard_scope import plan_header_guard_renames
from norminette_fix.header import ensure_header_guard
from norminette_fix.models import Identity

IDENTITY = Identity("vncosta", "vncosta@student.42sp.org", "test")


def engine(*, write: bool = False) -> FixEngine:
    return FixEngine(
        identity=IDENTITY,
        options=EngineOptions(write=write, backup=False),
    )


def initialize_git(path: Path) -> None:
    subprocess.run(
        ["git", "init", "-q", str(path)],
        check=True,
    )


def guarded_source(guard: str, function: str) -> str:
    return f"#ifndef {guard}\n# define {guard}\n\nint\t{function}(void);\n\n#endif\n"


def test_single_explicit_header_repairs_isolated_guard_pair(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "include" / "ft_printf.h"
    path.parent.mkdir()
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")

    result = engine(write=True).process([path])[0]
    fixed = path.read_text(encoding="utf-8")

    assert result.failure is None
    assert result.wrote
    assert "#ifndef FT_PRINTF_H" in fixed
    assert "# define FT_PRINTF_H" in fixed
    assert "FT_PRINT_H" not in fixed
    assert fixed.count("FT_PRINTF_H") == 2
    assert result.diagnostics_after == []
    assert any(
        fix.code == "HEADER_PROTECTION" and "project-wide reference check" in fix.description
        for fix in result.fixes
    )


def test_non_git_directory_keeps_guard_manual_without_a_proven_project_root(
    tmp_path: Path,
) -> None:
    path = tmp_path / "include" / "ft_printf.h"
    path.parent.mkdir()
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed
    assert any(
        diagnostic.code == "HEADER_PROTECTION_REVIEW" for diagnostic in result.diagnostics_after
    )


def test_unrequested_project_file_referencing_old_guard_blocks_rename(
    tmp_path: Path,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "include" / "ft_printf.h"
    path.parent.mkdir()
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / ".gitignore").write_text("*.flags\n", encoding="utf-8")
    (tmp_path / "build.flags").write_text("CPPFLAGS += -DFT_PRINT_H\n", encoding="utf-8")

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed
    assert not any(fix.code == "HEADER_PROTECTION" for fix in result.fixes)
    assert any(
        diagnostic.code == "HEADER_PROTECTION_REVIEW" for diagnostic in result.diagnostics_after
    )


def test_spliced_project_reference_to_old_guard_blocks_rename(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "include" / "ft_printf.h"
    path.parent.mkdir()
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "consumer.c").write_text(
        "#if defined(FT_PRINT_\\\nH)\nint\tconsumer(void);\n#endif\n",
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


def test_bare_cr_spliced_reference_to_old_guard_blocks_rename(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "consumer.c").write_bytes(
        b"#if defined(FT_PRINT_\\\rH)\rint\tconsumer(void);\r#endif\r"
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


@pytest.mark.parametrize(
    "definition",
    [
        "#define JOIN(left, right) left ## right\n",
        "%:define JOIN(left, right) left %:%: right\n",
        "??=define JOIN(left, right) left ??=??= right\n",
        "#define JOIN(left, right) left \\\n## right\n",
    ],
)
def test_token_pasting_anywhere_in_project_keeps_guard_manual(
    tmp_path: Path,
    definition: str,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "macros.h").write_text(definition, encoding="utf-8")

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


@pytest.mark.parametrize(
    "definition",
    [
        "CPPFLAGS += '-DCAT_RAW(a,b)=a##b'\n",
        'CPPFLAGS += "/DCAT_RAW(a,b)=a%:%:b"\n',
        "CPPFLAGS += -D 'CAT_RAW(a,b)=a??=??=b'\n",
        "CPPFLAGS += '-DCAT_RAW(a,b)=a\\#\\#b'\n",
        "CPPFLAGS += '-DCAT_RAW\\(a,b\\)=a\\#\\#b'\n",
    ],
)
def test_build_flag_token_pasting_keeps_guard_manual(
    tmp_path: Path,
    definition: str,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "build.flags").write_text(definition, encoding="utf-8")

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


def test_implicit_cmake_compile_definition_with_token_paste_blocks_rename(
    tmp_path: Path,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "CMakeLists.txt").write_text(
        'target_compile_definitions(app PRIVATE "CAT_RAW(a,b)=a##b")\n',
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


def test_dynamic_cmake_macro_definition_without_literal_name_blocks_rename(
    tmp_path: Path,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "CMakeLists.txt").write_text(
        'set(PREFIX "FT_PRINT_")\ntarget_compile_definitions(app PRIVATE "${PREFIX}H")\n',
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


def test_dynamic_make_compiler_definition_without_literal_name_blocks_rename(
    tmp_path: Path,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "Makefile").write_text(
        "PREFIX = FT_PRINT_\nCPPFLAGS += -D$(PREFIX)H\n",
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


def test_plain_42_makefile_warning_flags_do_not_block_rename(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "Makefile").write_text(
        "CFLAGS = -Wall -Wextra -Werror\n",
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINTF_H" in result.fixed
    assert result.diagnostics_after == []


@pytest.mark.parametrize(
    ("filename", "definition"),
    [
        ("defs.bzl", 'defines = ["CAT_RAW(a,b)=a##b"]\n'),
        (
            "config.xcconfig",
            "GCC_PREPROCESSOR_DEFINITIONS = CAT_RAW(a,b)=a##b\n",
        ),
        ("xmake.lua", 'add_defines("CAT_RAW(a,b)=a##b")\n'),
    ],
)
def test_implicit_build_definition_formats_with_token_paste_block_rename(
    tmp_path: Path,
    filename: str,
    definition: str,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / filename).write_text(definition, encoding="utf-8")

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


@pytest.mark.parametrize(
    ("filename", "comment"),
    [
        ("Makefile", "# Updated: test by user ###   ########.fr\n"),
        ("Makefile.local", "all: ## Build the project\n"),
        ("CMakeLists.txt", "# CAT_RAW(a,b)=a##b is only documentation\n"),
        (
            "CMakeLists.txt",
            "# target_compile_definitions(app PRIVATE CAT_RAW=a##b) is documentation\n",
        ),
    ],
)
def test_token_paste_text_in_build_comments_does_not_block_rename(
    tmp_path: Path,
    filename: str,
    comment: str,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / filename).write_text(comment, encoding="utf-8")

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINTF_H" in result.fixed
    assert result.diagnostics_after == []


def test_token_paste_text_in_comments_and_literals_does_not_block_rename(
    tmp_path: Path,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "comments.c").write_text(
        "/* ### and left ## right are only comments. */\n"
        'char\t*message(void) { return ("## %:%: ??="); }\n',
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINTF_H" in result.fixed
    assert result.diagnostics_after == []


def test_markdown_and_script_double_hashes_do_not_block_rename(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "README.md").write_text("## Build\n", encoding="utf-8")
    (tmp_path / "script.py").write_text("## section\n", encoding="utf-8")

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINTF_H" in result.fixed
    assert result.diagnostics_after == []


def test_existing_expected_macro_anywhere_in_project_blocks_rename(
    tmp_path: Path,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (tmp_path / "feature.c").write_text(
        "#ifdef FT_PRINTF_H\nint\tfeature(void);\n#endif\n",
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed
    assert any(
        diagnostic.code == "HEADER_PROTECTION_REVIEW" for diagnostic in result.diagnostics_after
    )


def test_multiple_selected_headers_receive_independent_approved_renames(
    tmp_path: Path,
) -> None:
    initialize_git(tmp_path)
    first = tmp_path / "alpha.h"
    second = tmp_path / "beta.h"
    first.write_text(guarded_source("ALPH_H", "alpha"), encoding="utf-8")
    second.write_text(guarded_source("BET_H", "beta"), encoding="utf-8")

    results = engine().process([first, second])

    assert len(results) == 2
    assert results[0].fixed is not None
    assert results[1].fixed is not None
    assert "#ifndef ALPHA_H" in results[0].fixed
    assert "#ifndef BETA_H" in results[1].fixed
    assert results[0].diagnostics_after == []
    assert results[1].diagnostics_after == []


def test_multiple_written_guard_repairs_revalidate_independently(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    first = tmp_path / "alpha.h"
    second = tmp_path / "beta.h"
    first.write_text(guarded_source("ALPH_H", "alpha"), encoding="utf-8")
    second.write_text(guarded_source("BET_H", "beta"), encoding="utf-8")

    results = engine(write=True).process([first, second])

    assert all(result.wrote for result in results)
    assert "#ifndef ALPHA_H" in first.read_text(encoding="utf-8")
    assert "#ifndef BETA_H" in second.read_text(encoding="utf-8")
    assert all(result.diagnostics_after == [] for result in results)


def test_known_source_write_advances_remaining_guard_approvals(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    source = tmp_path / "main.c"
    header = tmp_path / "beta.h"
    source.write_text("int\tmain(void)\n{\n\treturn (0);\n}\n", encoding="utf-8")
    header.write_text(guarded_source("BET_H", "beta"), encoding="utf-8")

    results = engine(write=True).process([source, header])

    assert all(result.wrote for result in results)
    assert "#ifndef BETA_H" in header.read_text(encoding="utf-8")
    assert all(result.diagnostics_after == [] for result in results)


def test_duplicate_expected_guard_within_one_project_is_ambiguous(
    tmp_path: Path,
) -> None:
    initialize_git(tmp_path)
    first = tmp_path / "first" / "config.h"
    second = tmp_path / "second" / "config.h"
    first.parent.mkdir()
    second.parent.mkdir()
    first.write_text(guarded_source("FIRST_CONFIG_H", "first_config"), encoding="utf-8")
    second.write_text(guarded_source("SECOND_CONFIG_H", "second_config"), encoding="utf-8")

    results = engine().process([first, second])

    assert results[0].fixed is not None
    assert results[1].fixed is not None
    assert "#ifndef FIRST_CONFIG_H" in results[0].fixed
    assert "#ifndef SECOND_CONFIG_H" in results[1].fixed
    assert "#ifndef CONFIG_H" not in results[0].fixed
    assert "#ifndef CONFIG_H" not in results[1].fixed


def test_unselected_duplicate_header_filename_blocks_rename(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    selected = tmp_path / "first" / "config.h"
    unselected = tmp_path / "second" / "config.h"
    selected.parent.mkdir()
    unselected.parent.mkdir()
    selected.write_text(guarded_source("FIRST_CONFIG_H", "first_config"), encoding="utf-8")
    unselected.write_text(
        guarded_source("SECOND_CONFIG_H", "second_config"),
        encoding="utf-8",
    )

    result = engine().process([selected])[0]

    assert result.fixed is not None
    assert "#ifndef FIRST_CONFIG_H" in result.fixed
    assert "#ifndef CONFIG_H" not in result.fixed
    assert any(
        diagnostic.code == "HEADER_PROTECTION_REVIEW" for diagnostic in result.diagnostics_after
    )


def test_header_in_tool_worktree_is_outside_the_project_scope(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    selected = tmp_path / "include" / "ft_printf.h"
    nested_root = tmp_path / ".claude" / "worktrees" / "review"
    selected.parent.mkdir()
    nested_root.mkdir(parents=True)
    initialize_git(nested_root)
    selected.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (nested_root / "ft_printf.h").write_text(
        guarded_source("NESTED_FT_PRINT_H", "nested_printf"),
        encoding="utf-8",
    )

    result = engine().process([selected])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINTF_H" in result.fixed
    assert result.diagnostics_after == []


def test_header_in_nested_vendor_repository_blocks_collision(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    selected = tmp_path / "include" / "ft_printf.h"
    nested_root = tmp_path / "vendor"
    selected.parent.mkdir()
    nested_root.mkdir()
    initialize_git(nested_root)
    selected.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (nested_root / "ft_printf.h").write_text(
        guarded_source("FT_PRINTF_H", "vendor_printf"),
        encoding="utf-8",
    )

    result = engine().process([selected])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


def test_symlinked_vendor_scope_fails_closed(tmp_path: Path) -> None:
    project = tmp_path / "project"
    external = tmp_path / "external"
    project.mkdir()
    external.mkdir()
    initialize_git(project)
    initialize_git(external)
    path = project / "ft_printf.h"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    (external / "ft_printf.h").write_text(
        guarded_source("FT_PRINTF_H", "external_printf"),
        encoding="utf-8",
    )
    (project / "vendor").symlink_to(external, target_is_directory=True)

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed


def test_same_filename_in_separate_git_projects_is_not_a_collision(
    tmp_path: Path,
) -> None:
    projects = [tmp_path / "one", tmp_path / "two"]
    paths: list[Path] = []
    for index, project in enumerate(projects, start=1):
        project.mkdir()
        initialize_git(project)
        path = project / "config.h"
        path.write_text(
            guarded_source(f"PROJECT_{index}_CONFIG_H", f"config_{index}"),
            encoding="utf-8",
        )
        paths.append(path)

    results = engine().process(paths)

    assert all(result.fixed is not None for result in results)
    assert all("#ifndef CONFIG_H" in (result.fixed or "") for result in results)
    assert all(result.diagnostics_after == [] for result in results)


def test_extra_reference_inside_canonical_header_keeps_guard_manual(
    tmp_path: Path,
) -> None:
    path = tmp_path / "ft_printf.h"
    path.write_text(
        "#ifndef FT_PRINT_H\n"
        "# define FT_PRINT_H\n"
        "\n"
        "# ifdef FT_PRINT_H\n"
        "int\tft_printf(void);\n"
        "# endif\n"
        "\n"
        "#endif\n",
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed
    assert any(
        diagnostic.code == "HEADER_PROTECTION_REVIEW" for diagnostic in result.diagnostics_after
    )


def test_guard_approval_is_bound_to_the_scanned_header_body(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    original = guarded_source("FT_PRINT_H", "ft_printf")
    path.write_text(original, encoding="utf-8")
    approval = plan_header_guard_renames([path.resolve()])[path.resolve()].rename
    changed_after_scan = original.replace(
        "int\tft_printf(void);",
        "# ifdef FT_PRINT_H\nint\tft_printf(void);\n# endif",
    )

    fixed, changed, _ = ensure_header_guard(
        changed_after_scan,
        path.name,
        approved_rename=approval,
    )

    assert not changed
    assert fixed == changed_after_scan
    assert fixed.count("FT_PRINT_H") == 3


def test_bom_header_is_normalized_and_renamed_in_one_run(tmp_path: Path) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    path.write_text(
        "\ufeff" + guarded_source("FT_PRINT_H", "ft_printf"),
        encoding="utf-8",
    )

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert not result.fixed.startswith("\ufeff")
    assert "#ifndef FT_PRINTF_H" in result.fixed
    assert "# define FT_PRINTF_H" in result.fixed
    assert result.diagnostics_after == []


def test_macos_private_var_alias_is_counted_once(tmp_path: Path) -> None:
    resolved = tmp_path.resolve()
    prefix = "/private/var/"
    if not str(resolved).startswith(prefix):
        pytest.skip("macOS /var compatibility alias is unavailable")
    initialize_git(resolved)
    real_path = resolved / "ft_printf.h"
    real_path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    alias_path = Path("/var") / real_path.relative_to("/private/var")

    result = engine().process([alias_path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINTF_H" in result.fixed
    assert result.diagnostics_after == []


def test_project_change_after_planning_invalidates_guard_approval(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    initialize_git(tmp_path)
    path = tmp_path / "ft_printf.h"
    consumer = tmp_path / "consumer.c"
    path.write_text(guarded_source("FT_PRINT_H", "ft_printf"), encoding="utf-8")
    consumer.write_text("int\tconsumer(void);\n", encoding="utf-8")
    real_plan = engine_module.plan_header_guard_renames

    def plan_then_change(paths: list[Path]):
        approvals = real_plan(paths)
        consumer.write_text(
            "#ifdef FT_PRINT_H\nint\tconsumer(void);\n#endif\n",
            encoding="utf-8",
        )
        return approvals

    monkeypatch.setattr(engine_module, "plan_header_guard_renames", plan_then_change)

    result = engine().process([path])[0]

    assert result.fixed is not None
    assert "#ifndef FT_PRINT_H" in result.fixed
    assert "#ifndef FT_PRINTF_H" not in result.fixed
    assert not any(fix.code == "HEADER_PROTECTION" for fix in result.fixes)
