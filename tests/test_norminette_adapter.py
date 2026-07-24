from pathlib import Path

import norminette_fix.norminette_adapter as adapter_module
from norminette_fix.norminette_adapter import NorminetteAdapter


class CountingRegistry:
    def __init__(self, registry) -> None:
        self.registry = registry
        self.calls = 0

    def run(self, context) -> None:
        self.calls += 1
        self.registry.run(context)


def test_lint_cache_returns_fresh_lists_without_rerunning_norminette(
    tmp_path: Path,
) -> None:
    path = tmp_path / "main.c"
    source = "int\tmain(void)\n{\n\treturn (0);\n}\n"
    adapter = NorminetteAdapter()
    registry = CountingRegistry(adapter._registry)
    adapter._registry = registry

    first, first_failure = adapter.lint(path, source)
    first.clear()
    second, second_failure = adapter.lint(path, source)

    assert registry.calls == 1
    assert first_failure is None
    assert second_failure is None
    assert second


def test_token_fingerprint_cache_avoids_relexing_same_source(
    tmp_path: Path,
    monkeypatch,
) -> None:
    path = tmp_path / "main.c"
    source = "int\tmain(void)\n{\n\treturn (0);\n}\n"
    adapter = NorminetteAdapter()
    registry = CountingRegistry(adapter._registry)
    adapter._registry = registry
    real_lexer = adapter_module.Lexer
    calls = 0

    def counting_lexer(norm_file):
        nonlocal calls
        calls += 1
        return real_lexer(norm_file)

    monkeypatch.setattr(adapter_module, "Lexer", counting_lexer)

    first = adapter.token_fingerprint(path, source)
    second = adapter.token_fingerprint(path, source)

    assert first == second
    assert calls == 1
    assert registry.calls == 1


def test_lint_and_token_fingerprint_share_one_analysis(
    tmp_path: Path,
    monkeypatch,
) -> None:
    path = tmp_path / "main.c"
    source = "int\tmain(void)\n{\n\treturn (0);\n}\n"
    adapter = NorminetteAdapter()
    registry = CountingRegistry(adapter._registry)
    adapter._registry = registry
    real_lexer = adapter_module.Lexer
    calls = 0

    def counting_lexer(norm_file):
        nonlocal calls
        calls += 1
        return real_lexer(norm_file)

    monkeypatch.setattr(adapter_module, "Lexer", counting_lexer)

    adapter.token_fingerprint(path, source)
    adapter.lint(path, source)

    assert calls == 1
    assert registry.calls == 1


def test_code_token_fingerprint_ignores_comments_but_not_code(tmp_path: Path) -> None:
    path = tmp_path / "main.c"
    adapter = NorminetteAdapter()
    with_comment = "int /* explanation */\tmain(void);\n"
    without_comment = "int\tmain(void);\n"
    different_code = "long\tmain(void);\n"

    assert adapter.code_token_fingerprint(
        path,
        with_comment,
    ) == adapter.code_token_fingerprint(path, without_comment)
    assert adapter.code_token_fingerprint(
        path,
        with_comment,
    ) != adapter.code_token_fingerprint(path, different_code)
