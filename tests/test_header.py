from datetime import datetime
from pathlib import Path

from norminette_fix.header import (
    HEADER_EDGE,
    build_header,
    ensure_header,
    ensure_header_guard,
    expected_guard,
    header_filename_matches,
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
    assert changed
    assert guard == expected_guard("ft_demo.h") == "FT_DEMO_H"
    assert "#ifndef FT_DEMO_H" in source
    assert "# define FT_DEMO_H" in source
    assert source.rstrip().endswith("#endif")


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
