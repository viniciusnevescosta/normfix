from datetime import datetime
from pathlib import Path

from norminette_fix.engine import EngineOptions, FixEngine
from norminette_fix.makefile import (
    analyze_makefile,
    build_makefile_header,
    compact_source_assignments,
    makefile_header_span,
)
from norminette_fix.models import Identity
from norminette_fix.source import visual_width

IDENTITY = Identity("student-a", "student-a@student.42.fr", "test")


def _body(source_assignment: str) -> str:
    return (
        "NAME\t= demo\n"
        f"{source_assignment}"
        "OBJ\t= $(SRC:.c=.o)\n"
        "\n"
        "all: $(NAME)\n"
        "\n"
        "$(NAME): $(OBJ)\n"
        "\t$(CC) $(OBJ) -o $(NAME)\n"
        "\n"
        "clean:\n"
        "\trm -f $(OBJ)\n"
        "\n"
        "fclean: clean\n"
        "\trm -f $(NAME)\n"
        "\n"
        "re: fclean all\n"
        "\n"
        ".PHONY: all clean fclean re\n"
    )


def test_makefile_header_has_the_official_shape() -> None:
    header = build_makefile_header(
        "Makefile",
        IDENTITY,
        now=datetime(2026, 6, 18, 15, 20, 13),
    )

    lines = header.splitlines()
    assert len(lines) == 11
    assert all(len(line) == 80 for line in lines)
    assert lines[0] == "# " + ("*" * 76) + " #"
    assert "By: student-a <student-a@student.42.fr>" in lines[5]
    assert makefile_header_span(header + "\n") is not None


def test_plain_source_list_is_greedily_packed_without_reordering() -> None:
    names = [f"source_{index:02d}.c" for index in range(18)]
    source = "SRC\t\t= \t" + " \\\n\t\t\t".join(names) + "\n"

    compacted, fixes = compact_source_assignments(source)

    assert fixes
    assert [token for token in compacted.replace("\\", "").split() if token.endswith(".c")] == names
    assert len(compacted.splitlines()) < len(source.splitlines())
    assert all(visual_width(line) <= 80 for line in compacted.splitlines())
    assert compact_source_assignments(compacted)[0] == compacted


def test_complex_make_constructs_are_never_reflowed() -> None:
    cases = (
        "SRC = $(addprefix src/,one.c two.c)\n",
        "SRC != printf 'one.c two.c'\n",
        "define TEMPLATE\nSRC = one.c \\\n two.c\nendef\n",
        "SRC = one.c two.c # selected by the subject\n",
        ".RECIPEPREFIX = >\nSRC = one.c \\\n two.c\n",
    )

    for source in cases:
        compacted, fixes = compact_source_assignments(source)
        assert compacted == source
        assert fixes == []


def test_makefile_analysis_reports_manual_norm_rules_in_english() -> None:
    source = "SRC = $(wildcard *.c)\nfirst:\n\tcc *.c\n"

    diagnostics = analyze_makefile(Path("Makefile"), source)
    codes = [diagnostic.code for diagnostic in diagnostics]

    assert "INVALID_HEADER" in codes
    assert "MAKEFILE_NAME_MISSING" in codes
    assert "MAKEFILE_WILDCARD_SOURCE" in codes
    assert "MAKEFILE_NAME_RULE_MISSING" in codes
    assert codes.count("MAKEFILE_MISSING_RULE") == 4
    assert "MAKEFILE_DEFAULT_RULE" in codes
    assert all(diagnostic.suggestion for diagnostic in diagnostics)


def test_engine_formats_makefile_without_sending_it_to_norminette(tmp_path: Path) -> None:
    class RefusingAdapter:
        def lint(self, _path: Path, _source: str):
            raise AssertionError("Makefiles must not be sent to Norminette")

    path = tmp_path / "Makefile"
    names = [f"file_{index:02d}.c" for index in range(15)]
    path.write_text(
        _body("SRC\t= " + " \\\n\t\t".join(names) + "\n"),
        encoding="utf-8",
    )
    engine = FixEngine(
        identity=IDENTITY,
        options=EngineOptions(write=False, backup=False),
        adapter=RefusingAdapter(),  # type: ignore[arg-type]
    )

    first = engine.process_file(path)

    assert first.failure is None
    assert first.fixed is not None
    assert first.fixed.startswith("# " + ("*" * 76) + " #")
    assert any(fix.code == "MAKEFILE_COMPACT_SOURCES" for fix in first.fixes)
    assert first.diagnostics_after == []
    assert "\t$(CC) $(OBJ) -o $(NAME)\n" in first.fixed

    path.write_text(first.fixed, encoding="utf-8")
    second = engine.process_file(path)
    assert not second.changed
    assert second.fixes == []
    assert second.diagnostics_after == []
