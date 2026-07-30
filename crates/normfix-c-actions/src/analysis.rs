//! Native structural diagnostics and actionable English guidance.

use camino::Utf8Path;
use normfix_c_syntax::{CFunctionKind, CParser};
use normfix_core::{Diagnostic, DiagnosticSource, Severity, TextRange, TextSize};

use crate::context::{ParsedContext, Token};
use crate::source::visual_width;
use crate::{CActionError, ReportedDiagnostic};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionInfo {
    pub(crate) name: String,
    pub(crate) name_start: usize,
    pub(crate) opening_line: u32,
    pub(crate) closing_line: u32,
    pub(crate) body_lines: u32,
    pub(crate) parameter_count: u32,
    pub(crate) variable_count: u32,
}

impl FunctionInfo {
    pub(crate) fn contains(&self, line: u32) -> bool {
        self.opening_line <= line && line <= self.closing_line
    }
}

/// Runs native structural checks on a clean C source.
///
/// # Errors
///
/// Returns an error if the parser cannot prove a lossless translation unit.
pub fn analyze_c(
    path: &Utf8Path,
    source: &str,
    max_columns: u32,
) -> Result<Vec<Diagnostic>, CActionError> {
    let mut parser = CParser::new()?;
    let context = ParsedContext::parse(&mut parser, source)?;
    context.require_safe()?;
    Ok(analyze_native(path, &context, max_columns))
}

pub(crate) fn analyze_native(
    path: &Utf8Path,
    context: &ParsedContext,
    max_columns: u32,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let functions = function_infos(context);
    let lines = context.lines();

    for (line_number, line, text) in lines.iter() {
        let width = visual_width(text);
        if width > max_columns {
            diagnostics.push(diagnostic(
                path,
                range(line.start, line.content_end),
                "LINE_TOO_LONG",
                format!("This line is {width} display column(s); the limit is {max_columns}."),
                Some(
                    "Split at a proven comma or binary operator while preserving strings, \
                     comments, and preprocessing semantics."
                        .to_owned(),
                ),
                vec![format!("physical line {line_number}")],
            ));
        }
    }

    for function in &functions {
        if function.body_lines > 25 {
            diagnostics.push(diagnostic(
                path,
                point(function.name_start),
                "TOO_MANY_LINES",
                format!(
                    "{}() has {} body line(s); the limit is 25.",
                    function.name, function.body_lines
                ),
                Some(
                    "Extract one coherent responsibility into a well-named static helper."
                        .to_owned(),
                ),
                Vec::new(),
            ));
        }
        if function.parameter_count > 4 {
            diagnostics.push(diagnostic(
                path,
                point(function.name_start),
                "TOO_MANY_ARGS",
                format!(
                    "{}() has {} parameter(s); the limit is 4.",
                    function.name, function.parameter_count
                ),
                Some(
                    "Reduce the contract to four parameters or group genuinely related state."
                        .to_owned(),
                ),
                Vec::new(),
            ));
        }
        if function.variable_count > 5 {
            diagnostics.push(diagnostic(
                path,
                point(function.name_start),
                "TOO_MANY_VARS_FUNC",
                format!(
                    "{}() declares {} local variable(s); the limit is 5.",
                    function.name, function.variable_count
                ),
                Some(
                    "Split the responsibility or simplify the local state declaration block."
                        .to_owned(),
                ),
                Vec::new(),
            ));
        }
    }

    if functions.len() > 5 {
        diagnostics.push(diagnostic(
            path,
            point(functions[5].name_start),
            "TOO_MANY_FUNCS",
            format!(
                "{} defines {} function(s); the limit is 5.",
                path.file_name().unwrap_or(path.as_str()),
                functions.len()
            ),
            Some(
                "Move a cohesive group of functions to another .c file and update interfaces and \
                 the Makefile."
                    .to_owned(),
            ),
            Vec::new(),
        ));
    }
    diagnostics
}

pub(crate) fn unsupported_reported_diagnostics(
    path: &Utf8Path,
    context: &ParsedContext,
    reported: &[ReportedDiagnostic],
) -> Vec<Diagnostic> {
    let lines = context.lines();
    reported
        .iter()
        .filter(|item| !is_supported_action(&item.code))
        .map(|item| {
            let location = lines.get(item.line).map_or_else(
                || point(0),
                |line| point(lines.byte_for_visual_column(line, item.visual_column)),
            );
            diagnostic(
                path,
                location,
                &item.code,
                if item.message.is_empty() {
                    "Norm rule requires manual review.".to_owned()
                } else {
                    item.message.clone()
                },
                Some(guidance(&item.code).to_owned()),
                vec![
                    "No semantics-preserving native action was proven for this diagnostic."
                        .to_owned(),
                ],
            )
        })
        .collect()
}

pub(crate) fn function_infos(context: &ParsedContext) -> Vec<FunctionInfo> {
    let lines = context.lines();
    context
        .facts()
        .functions
        .iter()
        .filter(|fact| fact.kind == CFunctionKind::Definition)
        .filter_map(|fact| {
            let body = fact.body_range?;
            let body_start = body.start().get() as usize;
            let body_end = body.end().get() as usize;
            let opening_line = lines.line_number_at(body_start);
            let closing_line = lines.line_number_at(body_end.saturating_sub(1));
            Some(FunctionInfo {
                name: fact.name.clone(),
                name_start: fact.name_range.start().get() as usize,
                opening_line,
                closing_line,
                body_lines: closing_line.saturating_sub(opening_line).saturating_sub(1),
                parameter_count: fact.parameter_count,
                variable_count: local_variable_count(context, opening_line, closing_line),
            })
        })
        .collect()
}

fn local_variable_count(context: &ParsedContext, opening: u32, closing: u32) -> u32 {
    let lines = context.lines();
    let mut count = 0_u32;
    for line_number in opening.saturating_add(1)..closing {
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line).trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        if text.starts_with('{') {
            continue;
        }
        if text.starts_with('}') {
            break;
        }
        if !looks_like_local_declaration(text) {
            break;
        }
        count = count.saturating_add(1 + top_level_commas(text));
    }
    count
}

fn looks_like_local_declaration(text: &str) -> bool {
    if !text.ends_with(';') {
        return false;
    }
    let first = text
        .split(|character: char| character.is_whitespace() || character == '*')
        .find(|word| !word.is_empty())
        .unwrap_or("");
    is_type_word(first)
        || first.starts_with("t_")
        || first.ends_with("_t")
        || first.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

fn top_level_commas(text: &str) -> u32 {
    let mut depth = 0_i32;
    let mut count = 0_u32;
    for character in text.chars() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            ',' if depth == 0 => count = count.saturating_add(1),
            _ => {}
        }
    }
    count
}

fn is_type_word(word: &str) -> bool {
    matches!(
        word,
        "auto"
            | "char"
            | "const"
            | "double"
            | "enum"
            | "extern"
            | "float"
            | "inline"
            | "int"
            | "long"
            | "register"
            | "restrict"
            | "short"
            | "signed"
            | "static"
            | "struct"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "_Atomic"
            | "_Bool"
    )
}

pub(crate) fn matching_forward(
    tokens: &[Token],
    opening: usize,
    opening_text: &str,
    closing_text: &str,
) -> Option<usize> {
    let mut depth = 0_u32;
    for (index, token) in tokens.iter().enumerate().skip(opening) {
        if token.text == opening_text {
            depth = depth.saturating_add(1);
        } else if token.text == closing_text {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

pub(crate) fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn is_supported_action(code: &str) -> bool {
    matches!(
        code,
        "NEWLINE_PRECEDES_FUNC"
            | "NL_AFTER_VAR_DECL"
            | "NL_AFTER_PREPROC"
            | "EMPTY_LINE_FUNCTION"
            | "CONSECUTIVE_NEWLINES"
            | "BRACE_NEWLINE"
            | "BRACE_SHOULD_EOL"
            | "EXP_NEWLINE"
            | "SPACE_BEFORE_FUNC"
            | "TOO_MANY_TABS_FUNC"
            | "MISSING_TAB_FUNC"
            | "MISALIGNED_FUNC_DECL"
            | "SPACE_REPLACE_TAB"
            | "TAB_REPLACE_SPACE"
            | "TOO_FEW_TAB"
            | "TOO_MANY_TAB"
            | "MIXED_SPACE_TAB"
            | "MISSING_TAB_VAR"
            | "MISSING_TAB_TYPDEF"
            | "NO_TAB_BF_TYPEDEF"
            | "SPC_BFR_OPERATOR"
            | "SPC_AFTER_OPERATOR"
            | "NO_SPC_BFR_OPR"
            | "NO_SPC_AFR_OPR"
            | "SPC_BFR_POINTER"
            | "SPC_AFTER_POINTER"
            | "SPC_BFR_PAR"
            | "SPC_AFTER_PAR"
            | "NO_SPC_BFR_PAR"
            | "NO_SPC_AFR_PAR"
            | "CONSECUTIVE_SPC"
            | "CONSECUTIVE_WS"
            | "TAB_INSTEAD_SPC"
            | "SPACE_AFTER_KW"
            | "SPC_LINE_START"
            | "MISALIGNED_VAR_DECL"
            | "LINE_TOO_LONG"
            | "RETURN_PARENTHESIS"
            | "NO_ARGS_VOID"
            | "WRONG_SCOPE_COMMENT"
            | "COMMENT_ON_INSTR"
    )
}

fn guidance(code: &str) -> &'static str {
    match code {
        "TOO_MANY_LINES" => "Extract one coherent responsibility into a well-named static helper.",
        "TOO_MANY_ARGS" => {
            "Reduce the contract to four parameters or group genuinely related state."
        }
        "TOO_MANY_VARS_FUNC" => {
            "Split the responsibility or simplify the state to at most five locals."
        }
        "TOO_MANY_FUNCS" => {
            "Move a cohesive group of functions and update prototypes and the Makefile."
        }
        "FORBIDDEN_CS" => "Rewrite the forbidden control structure, usually as a while loop.",
        "TERNARY_FBIDDEN" => "Replace the ternary with an explicit if/else.",
        "GOTO_FBIDDEN" | "LABEL_FBIDDEN" => {
            "Restructure the associated control flow without goto or labels."
        }
        "VLA_FORBIDDEN" => {
            "Use a proven compile-time constant bound or an allowed allocation strategy."
        }
        "ASSIGN_IN_CONTROL" => "Move the assignment to its own instruction.",
        "DECL_ASSIGN_LINE" => {
            "Declare the local in the initial declaration block, then assign it separately."
        }
        "MULT_DECL_LINE" => "Write exactly one variable declaration per line.",
        "VAR_DECL_START_FUNC" => {
            "Move the declaration into the function's initial declaration block."
        }
        "MULT_ASSIGN_LINE" => "Split chained assignments while preserving their evaluation order.",
        "FORBIDDEN_CHAR_NAME" => {
            "Rename the symbol project-wide and check every scope for collisions."
        }
        "GLOBAL_VAR_NAMING" => "Rename the global project-wide with the required g_ prefix.",
        "USER_DEFINED_TYPEDEF" => "Rename the typedef project-wide with the required t_ prefix.",
        "STRUCT_TYPE_NAMING" => "Rename the structure tag with the required s_ prefix.",
        "ENUM_TYPE_NAMING" => "Rename the enum tag with the required e_ prefix.",
        "UNION_TYPE_NAMING" => "Rename the union tag with the required u_ prefix.",
        "MACRO_NAME_CAPITAL" => "Rename the macro and every use to uppercase.",
        "GLOBAL_VAR_DETECTED" => {
            "Confirm the global is allowed, const/static, and justified by the project."
        }
        "INCLUDE_START_FILE" => {
            "Move the include to the include block after checking conditional dependencies."
        }
        "INCLUDE_HEADER_ONLY" => "Include a .h interface rather than a .c implementation.",
        "PREPROC_MULTLINE" => "Replace the forbidden multiline macro deliberately.",
        "PREPOC_ONLY_GLOBAL" | "PREPROC_GLOBAL" => {
            "Move the preprocessing directive to global scope."
        }
        "FORBIDDEN_STRUCT" | "FORBIDDEN_ENUM" | "FORBIDDEN_UNION" | "FORBIDDEN_TYPEDEF" => {
            "Move the type definition to an appropriate header."
        }
        "HEADER_PROT_NAME" | "HEADER_PROT_ALL" | "HEADER_PROT_ALL_AF" => {
            "Review repeat-inclusion behavior and every project-wide macro reference first."
        }
        _ => "Review this location and apply the named Norm rule manually.",
    }
}

fn diagnostic(
    path: &Utf8Path,
    range: TextRange,
    rule_id: impl Into<String>,
    message: impl Into<String>,
    help: Option<String>,
    notes: Vec<String>,
) -> Diagnostic {
    Diagnostic {
        rule_id: rule_id.into(),
        path: path.to_owned(),
        range,
        severity: Severity::Error,
        message: message.into(),
        source: DiagnosticSource::NativeNorm41,
        notes,
        help,
    }
}

fn range(start: usize, end: usize) -> TextRange {
    let start = TextSize::try_from(start).unwrap_or(TextSize::new(u32::MAX));
    let end = TextSize::try_from(end).unwrap_or(TextSize::new(u32::MAX));
    TextRange::new(start, end).unwrap_or_else(|| TextRange::empty(start))
}

fn point(offset: usize) -> TextRange {
    range(offset, offset)
}
