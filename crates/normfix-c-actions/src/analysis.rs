//! Native structural diagnostics and actionable English guidance.

use camino::{Utf8Path, Utf8PathBuf};
use normfix_c_syntax::{CFunctionKind, CParser, CTypeTagKind};
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
    pub(crate) line: u32,
}

/// Read-only per-function Norm budget data for terminal summaries and the
/// future `normfix budget` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionBudget {
    /// File supplied to the analysis API.
    pub path: Utf8PathBuf,
    /// Function identifier.
    pub function: String,
    /// One-based line containing the function identifier.
    pub line: u32,
    /// Physical lines between the function braces.
    pub lines: u32,
    /// Norm v4 function body line limit.
    pub line_limit: u32,
    /// Locals in the initial declaration block.
    pub variables: u32,
    /// Norm v4 local variable limit.
    pub variable_limit: u32,
    /// Declared parameters.
    pub parameters: u32,
    /// Norm v4 parameter limit.
    pub parameter_limit: u32,
}

/// A simple call that remains a candidate for project allowlist policy after
/// file-local definitions, parameters, locals, macros, and ambiguous regions
/// have been removed conservatively.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalCallCandidate {
    /// File supplied to the analysis API.
    pub path: Utf8PathBuf,
    /// Simple callee identifier.
    pub name: String,
    /// Exact identifier range.
    pub name_range: TextRange,
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

/// Returns function budgets without adding them to normal diagnostics.
///
/// # Errors
///
/// Returns an error if the embedded parser cannot prove a lossless source.
pub fn analyze_budget(path: &Utf8Path, source: &str) -> Result<Vec<FunctionBudget>, CActionError> {
    let mut parser = CParser::new()?;
    let context = ParsedContext::parse(&mut parser, source)?;
    context.require_safe()?;
    Ok(function_infos(&context)
        .into_iter()
        .map(|function| FunctionBudget {
            path: path.to_owned(),
            function: function.name,
            line: function.line,
            lines: function.body_lines,
            line_limit: 25,
            variables: function.variable_count,
            variable_limit: 5,
            parameters: function.parameter_count,
            parameter_limit: 4,
        })
        .collect())
}

/// Returns conservative external-call candidates for one source file.
///
/// This API deliberately applies no subject allowlist. Calls shadowed by a
/// recoverable parameter/local, resolved to a definition in the same file, or
/// entangled with preprocessing are omitted. Token paste or an ambiguous local
/// declaration fails closed by suppressing affected candidates.
///
/// # Errors
///
/// Returns an error if the source cannot be parsed losslessly.
pub fn analyze_external_calls(
    path: &Utf8Path,
    source: &str,
) -> Result<Vec<ExternalCallCandidate>, CActionError> {
    let mut parser = CParser::new()?;
    let context = ParsedContext::parse(&mut parser, source)?;
    context.require_safe()?;
    if context.tokens().iter().any(|token| token.text == "##") {
        return Ok(Vec::new());
    }
    let mut candidates = context
        .facts()
        .calls
        .iter()
        .filter(|call| external_call_is_proven(&context, call))
        .map(|call| ExternalCallCandidate {
            path: path.to_owned(),
            name: call.name.clone(),
            name_range: call.name_range,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (&left.path, left.name_range, &left.name).cmp(&(&right.path, right.name_range, &right.name))
    });
    candidates.dedup();
    Ok(candidates)
}

fn external_call_is_proven(context: &ParsedContext, call: &normfix_c_syntax::CallFact) -> bool {
    if !call
        .name
        .chars()
        .any(|character| character.is_ascii_lowercase())
        || context
            .facts()
            .macros
            .iter()
            .any(|macro_fact| macro_fact.name == call.name)
        || context
            .facts()
            .preprocessor_ranges
            .iter()
            .any(|range| range.contains(call.name_range.start()))
        || context.facts().functions.iter().any(|function| {
            function.kind == CFunctionKind::Definition && function.name == call.name
        })
    {
        return false;
    }
    let Some(owner) = context.facts().functions.iter().find(|function| {
        function.kind == CFunctionKind::Definition
            && function.range.contains(call.name_range.start())
    }) else {
        return false;
    };
    if owner
        .parameters
        .iter()
        .any(|parameter| parameter.name == call.name)
    {
        return false;
    }
    let visible_locals = context
        .facts()
        .local_declarations
        .iter()
        .filter(|declaration| declaration.function_name == owner.name)
        .filter(|declaration| declaration.range.start() <= call.name_range.start())
        .filter(|declaration| declaration.scope_range.contains(call.name_range.start()))
        .collect::<Vec<_>>();
    if visible_locals
        .iter()
        .any(|declaration| declaration.name.as_deref() == Some(call.name.as_str()))
    {
        return false;
    }
    !visible_locals
        .iter()
        .any(|declaration| local_declaration_is_ambiguous(context, declaration))
}

fn local_declaration_is_ambiguous(
    context: &ParsedContext,
    declaration: &normfix_c_syntax::LocalDeclarationFact,
) -> bool {
    if declaration.name.is_none() {
        return true;
    }
    let range = declaration.range;
    let mut parentheses = 0_u32;
    let mut brackets = 0_u32;
    let mut braces = 0_u32;
    for token in context.tokens().iter().filter(|token| {
        token.start >= range.start().get() as usize && token.end <= range.end().get() as usize
    }) {
        match token.text.as_str() {
            "(" => parentheses = parentheses.saturating_add(1),
            ")" => parentheses = parentheses.saturating_sub(1),
            "[" => brackets = brackets.saturating_add(1),
            "]" => brackets = brackets.saturating_sub(1),
            "{" => braces = braces.saturating_add(1),
            "}" => braces = braces.saturating_sub(1),
            "," if parentheses == 0 && brackets == 0 && braces == 0 => return true,
            _ => {}
        }
    }
    false
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
    append_include_order_diagnostics(path, context, &mut diagnostics);
    append_review_diagnostics(path, context, &mut diagnostics);
    diagnostics
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct IncludeOrderKey {
    category: u8,
    name: String,
}

fn append_include_order_diagnostics(
    path: &Utf8Path,
    context: &ParsedContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut group = Vec::<(usize, IncludeOrderKey)>::new();
    for (_, line, text) in context.lines().iter() {
        if let Some(key) = include_order_key(text) {
            group.push((line.start, key));
            continue;
        }
        append_include_group_diagnostic(path, &group, diagnostics);
        group.clear();
    }
    append_include_group_diagnostic(path, &group, diagnostics);
}

fn append_include_group_diagnostic(
    path: &Utf8Path,
    group: &[(usize, IncludeOrderKey)],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if group.len() < 2 || group.windows(2).all(|pair| pair[0].1 <= pair[1].1) {
        return;
    }
    diagnostics.push(review_diagnostic(
        path,
        point(group[0].0),
        "INCLUDE_ORDER_REVIEW",
        "This contiguous include block is not ordered with system headers first and each group alphabetically.",
        "Review and reorder the block manually; expected display order is <system headers>, then \"project headers\", alphabetically within each category. Changing preprocessor order can alter declarations, feature macros, and conditional compilation, so normfix does not guess.",
    ));
}

fn include_order_key(text: &str) -> Option<IncludeOrderKey> {
    let directive = text.trim().strip_prefix('#')?.trim_start();
    let rest = directive.strip_prefix("include")?;
    if rest
        .chars()
        .next()
        .is_some_and(|character| !character.is_ascii_whitespace())
    {
        return None;
    }
    let operand = rest.trim();
    let (category, closing) = match operand.as_bytes().first().copied()? {
        b'<' => (0, '>'),
        b'"' => (1, '"'),
        _ => return None,
    };
    let body = operand.get(1..)?;
    let end = body.find(closing)?;
    if !body.get(end + 1..)?.trim().is_empty() {
        return None;
    }
    Some(IncludeOrderKey {
        category,
        name: body[..end].to_ascii_lowercase(),
    })
}

fn append_review_diagnostics(
    path: &Utf8Path,
    context: &ParsedContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in context
        .facts()
        .local_declarations
        .iter()
        .filter(|declaration| !declaration.initial)
    {
        diagnostics.push(review_diagnostic(
            path,
            declaration.name_range.unwrap_or(declaration.range),
            "LATE_DECLARATION_REVIEW",
            format!(
                "A local declaration in {}() appears after the initial declaration block.",
                declaration.function_name
            ),
            "Move a proven plain declaration to the top of the function, but keep its assignment at the original execution point.",
        ));
    }
    for tag in &context.facts().type_tags {
        let (prefix, rule, label) = match tag.kind {
            CTypeTagKind::Struct => ("s_", "STRUCT_TYPE_NAMING_REVIEW", "struct"),
            CTypeTagKind::Union => ("u_", "UNION_TYPE_NAMING_REVIEW", "union"),
            CTypeTagKind::Enum => ("e_", "ENUM_TYPE_NAMING_REVIEW", "enum"),
        };
        if !tag.name.starts_with(prefix) {
            diagnostics.push(review_diagnostic(
                path,
                tag.name_range,
                rule,
                format!("The {label} tag `{}` does not start with `{prefix}`.", tag.name),
                "Rename the tag and every bound reference project-wide after checking scopes, macros, and external interfaces.",
            ));
        }
    }
    for loop_fact in context
        .facts()
        .loops
        .iter()
        .filter(|loop_fact| loop_fact.unconditional && !loop_fact.has_obvious_exit)
    {
        diagnostics.push(review_diagnostic(
            path,
            loop_fact.range,
            "POSSIBLE_INFINITE_LOOP",
            "This loop is syntactically unconditional and has no obvious return, goto, or owning break.",
            "Confirm that state changed in the body eventually reaches an intentional exit; static syntax alone cannot prove termination.",
        ));
    }
    append_pointer_return_reviews(path, context, diagnostics);
}

fn append_pointer_return_reviews(
    path: &Utf8Path,
    context: &ParsedContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for returned in context
        .facts()
        .returns
        .iter()
        .filter(|returned| returned.function_returns_pointer)
    {
        let Some(expression) = returned.expression_range else {
            continue;
        };
        let start = expression.start().get() as usize;
        let end = expression.end().get() as usize;
        let expression_text = context.source().get(start..end).unwrap_or("");
        let compact = expression_text
            .chars()
            .filter(|character| !character.is_whitespace() && !matches!(character, '(' | ')'))
            .collect::<String>();
        if compact == "0" && !context.null_is_proven_available_at(start) {
            diagnostics.push(review_diagnostic(
                path,
                expression,
                "POINTER_ZERO_RETURN_REVIEW",
                format!(
                    "{}() returns pointer constant 0, but no prior proven NULL provider is present.",
                    returned.function_name
                ),
                "Make NULL available with an appropriate standard header before rewriting this return as NULL.",
            ));
        }
    }
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
                variable_count: local_variable_count(context, &fact.name),
                line: lines.line_number_at(fact.name_range.start().get() as usize),
            })
        })
        .collect()
}

fn local_variable_count(context: &ParsedContext, function_name: &str) -> u32 {
    context
        .facts()
        .local_declarations
        .iter()
        .filter(|declaration| declaration.function_name == function_name)
        .filter_map(|declaration| {
            let start = declaration.range.start().get() as usize;
            let end = declaration.range.end().get() as usize;
            context.source().get(start..end)
        })
        .fold(0_u32, |count, declaration| {
            count.saturating_add(1 + top_level_commas(declaration))
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

fn review_diagnostic(
    path: &Utf8Path,
    range: TextRange,
    rule_id: impl Into<String>,
    message: impl Into<String>,
    help: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        rule_id: rule_id.into(),
        path: path.to_owned(),
        range,
        severity: Severity::Warning,
        message: message.into(),
        source: DiagnosticSource::NativeNorm41,
        notes: vec!["Review required; no project-wide semantic edit was applied.".to_owned()],
        help: Some(help.into()),
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
