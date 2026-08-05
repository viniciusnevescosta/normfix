//! Short, offline explanations for the rules most often requiring review.

pub(crate) fn explain(rule: &str) -> Option<String> {
    let canonical = rule.trim().to_ascii_uppercase();
    if canonical.is_empty()
        || !canonical.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return None;
    }
    if canonical == "CC_ANALYZER_UNAVAILABLE" {
        return Some(formatted(
            &canonical,
            "The requested analyzer is not available",
            "--analyzer was requested, but the selected compiler ships neither GCC -fanalyzer nor the Clang analyzer, so the deep pass was skipped. Nothing was analyzed and nothing failed.",
            "Point --cc at a real GCC or Clang, or drop --analyzer. On macOS, /usr/bin/gcc is Clang under another name, and normfix already uses the Clang analyzer for it.",
            "This is informational and fail-open: a missing analyzer never changes the exit status and never blocks a fix.",
        ));
    }
    if canonical.starts_with("CC_ANALYZER_") {
        return Some(formatted(
            &canonical,
            "Optional static-analyzer finding",
            "GCC -fanalyzer found a path worth investigating; it is not a complete proof of a leak or invalid access.",
            "Inspect the compiler location, reproduce the path with tests, and confirm ownership with a runtime tool when available.",
            "Analyzer output is opt-in, informational, fail-open, and never authorizes a rewrite.",
        ));
    }
    if canonical.starts_with("CC_") {
        return Some(formatted(
            &canonical,
            "Strict compiler preflight finding",
            "The real project source was checked with -fsyntax-only -Wall -Wextra -Werror and the compiler reported this issue.",
            "Follow the compiler location and message, then run the project Makefile separately with the subject's required toolchain.",
            "Compiler diagnostics are read-only and never authorize source edits.",
        ));
    }
    let (title, why, next, safety) = article(canonical.as_str());
    Some(formatted(&canonical, title, why, next, safety))
}

/// One bundled explanation: title, why, next step, and safety note.
type Article = (&'static str, &'static str, &'static str, &'static str);

fn article(canonical: &str) -> Article {
    structural_article(canonical)
        .or_else(|| toolchain_article(canonical))
        .unwrap_or((
            "Rule reported by an analysis backend",
            "No dedicated long-form article is bundled for this identifier; the normal diagnostic includes the authoritative message, location, source, and contextual help.",
            "Run normfix again with --verbose, inspect the highlighted source, and apply the diagnostic's Next/help guidance.",
            "An unknown explanation never enables an automatic edit. Edits still require their normal structural and oracle proofs.",
        ))
}

/// Explanations for limits the 42 Norm places on source structure.
fn structural_article(canonical: &str) -> Option<Article> {
    Some(match canonical {
        "TOO_MANY_LINES" => (
            "Function body exceeds 25 lines",
            "The 42 Norm limits each function body to 25 physical lines so responsibilities stay small and reviewable.",
            "Extract one coherent responsibility. Keep live inputs to four parameters or fewer and verify that the file still contains at most five functions.",
            "normfix reports this as a suggestion because choosing a function boundary changes program structure.",
        ),
        "TOO_MANY_ARGS" => (
            "Function has more than four parameters",
            "A Norm-compliant function may receive at most four named parameters.",
            "Narrow the contract or group genuinely related state in an existing project type; do not create a meaningless wrapper only to hide the count.",
            "Changing a public signature is an API change, so it is never applied automatically.",
        ),
        "TOO_MANY_VARS_FUNC" => (
            "Function declares more than five local variables",
            "The limit includes variables declared in the function's initial declaration block.",
            "Remove redundant state or extract a cohesive operation. Moving declarations alone does not reduce the count.",
            "Automatic extraction would require human naming and ownership decisions.",
        ),
        "TOO_MANY_FUNCS" => (
            "File defines more than five functions",
            "The 42 Norm limits the number of function definitions in one C source file.",
            "Move a cohesive group to another .c file, then update its header and the Makefile together.",
            "Cross-file refactors are suggestions until project-wide linkage and build proofs succeed.",
        ),
        "LINE_TOO_LONG" => (
            "Line exceeds 80 display columns",
            "Tabs use four-column stops and wide Unicode characters can consume more than one terminal column.",
            "Break at a proven comma or binary operator. Strings, comments, macros, unary operators, and evaluation order are protected barriers.",
            "normfix applies only token-preserving wraps and leaves every ambiguous line for review.",
        ),
        "VAR_DECL_START_FUNC"
        | "DECL_ASSIGN_LINE"
        | "NL_AFTER_VAR_DECL"
        | "MISALIGNED_VAR_DECL"
        | "TOO_MANY_TAB"
        | "TOO_FEW_TAB" => (
            "Local declaration block is not canonical",
            "Local declarations belong at the beginning of the function or block, one declaration per line, followed by one blank line before instructions.",
            "Move the declaration without its assignment, then assign shortly before the first use.",
            "Hoisting can change scope or lifetime, so only structurally proven moves are automatic.",
        ),
        "HEADER_PROT_NAME" | "HEADER_PROT_NODEF" | "HEADER_PROTECTION_REVIEW" => (
            "Header inclusion guard is incomplete or inconsistent",
            "The #ifndef and #define names must match the filename-derived guard and the final #endif must protect the whole header.",
            "Use one outer guard and check for project-wide macro references, #undef, X-macro use, and repeated-inclusion behavior.",
            "Guard edits are accepted only with a closed-project collision proof and final Norminette validation.",
        ),
        "INCLUDE_ORDER" | "INCLUDE_ORDER_REVIEW" => (
            "Include block order",
            "The expected display order is <system headers> first, then \"project headers\", alphabetically inside each category.",
            "Nothing to do when a fixing run reordered the block; reorder by hand when the report kept it, which happens with --no-reorder-includes or when a comment, conditional, or macro interrupts the run of directives.",
            "A block is rewritten only when every one of its lines is exactly one include directive, so no directive is ever moved across a construct that could change what a header means.",
        ),
        _ => return None,
    })
}

/// Explanations for findings produced by the toolchain and project policy.
fn toolchain_article(canonical: &str) -> Option<Article> {
    Some(match canonical {
        "MAKEFILE_SOURCE_NOT_FOUND" | "MISSING_MAKEFILE_SOURCE" => (
            "Makefile references a missing C source",
            "A literal source listed in SRCS/SRC must resolve inside the project or the build will fail.",
            "Restore the file or remove the exact literal token. Dynamic Make expressions are intentionally not guessed.",
            "Removal is destructive and therefore requires --unsafe plus confirmation or --force.",
        ),
        "COMPILER_WARNING" | "COMPILER_PREFLIGHT" => (
            "Strict compiler preflight failed",
            "42 projects are expected to compile with -Wall -Wextra -Werror; -Werror promotes every emitted warning to an error.",
            "Follow the compiler location and message. This check does not claim to detect memory leaks.",
            "Compiler diagnostics are read-only and never authorize a source rewrite.",
        ),
        "ANALYZER_WARNING" => (
            "Optional static analyzer finding",
            "GCC -fanalyzer explores paths and may report leaks or invalid accesses, but it is incomplete across translation units and ownership stored in structs.",
            "Treat the finding as evidence to investigate, then confirm with project tests and runtime tools.",
            "Analyzer output is informational, opt-in, and never part of the fix proof gate.",
        ),
        "STATIC_HELPER_CANDIDATE" => (
            "Project-local helper may be eligible for static linkage",
            "A function used only inside its translation unit should normally have internal linkage.",
            "Confirm it is absent from public headers, callbacks, generated references, macros, and other translation units before adding static.",
            "Linkage changes are suggestions unless a complete project graph proves them.",
        ),
        "C_PARSER_FAILURE" | "PARSER_RECOVERY" => (
            "C parser recovered from invalid or unsupported syntax",
            "A missing or extra token makes the intended program ambiguous, so formatting through the recovery region could corrupt code.",
            "Repair the syntax at the reported location and run normfix again.",
            "normfix never guesses arbitrary parentheses, braces, semicolons, or operators.",
        ),
        "FUNCTION_NOT_ALLOWED" => (
            "Call is absent from the project allowlist",
            "normfix.toml declares the external functions permitted by the current 42 subject, and this recoverable direct call is not listed.",
            "Check the subject, then remove the call or add its exact name to [project].allowed only when it is genuinely authorized.",
            "Macros, local definitions, parameters, locals, and ambiguous function-pointer calls are excluded conservatively.",
        ),
        "NORM_BUDGET" | "FUNCTION_BUDGET" => (
            "Per-function Norm headroom",
            "This informational row shows current body lines, local variables, and parameters against the 25/5/4 limits.",
            "Keep some headroom for defense-day changes; exceeded limits also appear as dedicated warnings.",
            "Budget reporting is read-only and automatic function extraction is intentionally not attempted.",
        ),
        "WRONG_SCOPE_COMMENT" | "COMMENT_ON_INSTR" => (
            "Comment placement is outside the accepted Norm scope",
            "The official oracle rejected a comment at this exact location.",
            "Move or rewrite the comment in English, or explicitly request removal when losing the comment is acceptable.",
            "Comments are only removed through the explicit opt-in path; ordinary formatting preserves them.",
        ),
        _ => return None,
    })
}

fn formatted(canonical: &str, title: &str, why: &str, next: &str, safety: &str) -> String {
    format!("{canonical}: {title}\n\nWhy\n  {why}\n\nNext\n  {next}\n\nSafety\n  {safety}\n")
}

#[cfg(test)]
mod tests {
    use super::explain;

    #[test]
    fn explanations_are_case_insensitive_and_unknown_rules_stay_safe() {
        let text = explain("line_too_long").expect("known rule");
        assert!(text.starts_with("LINE_TOO_LONG"));
        assert!(text.contains("80 display columns"));
        let fallback = explain("NOT_A_REAL_RULE").expect("safe generic explanation");
        assert!(fallback.contains("No dedicated long-form article"));
        assert!(explain("not a rule").is_none());
    }
}
