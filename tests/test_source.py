from norminette_fix.source import (
    normalize_hygiene,
    visual_column_to_index,
    visual_width,
)


def test_visual_columns_expand_tabs_to_four_column_stops() -> None:
    assert visual_width("\tfoo") == 7
    assert visual_column_to_index("\tfoo", 5) == 1


def test_hygiene_cleanup_is_idempotent() -> None:
    source = "\n\nint x;   \n\n\n\n"
    fixed, fixes = normalize_hygiene(source)
    second, second_fixes = normalize_hygiene(fixed)
    assert fixed == "int x;\n"
    assert fixes
    assert second == fixed
    assert second_fixes == []


def test_trailing_space_after_backslash_is_not_stripped() -> None:
    source = "#define BAD value\\   \n"
    fixed, _ = normalize_hygiene(source)
    assert fixed == source
