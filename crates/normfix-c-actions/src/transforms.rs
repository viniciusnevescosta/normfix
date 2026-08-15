//! Ordered native C transformation phases.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use normfix_c_syntax::{CFunctionKind, TernaryForm};
use normfix_core::TextRange;
use regex::Regex;

use crate::analysis::{
    IncludeOrderKey, function_infos, include_order_key, is_identifier, matching_forward,
};
use crate::context::{ParsedContext, Token};
use crate::edit::Edit;
use crate::source::{
    LexicalMap, PhysicalLine, SourceLines, escaped_physical_newline, leading_whitespace,
    visual_width, visual_width_from, whitespace_after, whitespace_before,
};
use crate::{Applicability, CActionError, CActionOptions, ReportedDiagnostic};

#[derive(Clone, Debug)]
pub(crate) struct ActionBatch {
    pub(crate) rule_id: &'static str,
    pub(crate) applicability: Applicability,
    pub(crate) edits: Vec<Edit>,
}

impl ActionBatch {
    fn layout(rule_id: &'static str, edits: Vec<Edit>) -> Option<Self> {
        (!edits.is_empty()).then_some(Self {
            rule_id,
            applicability: Applicability::SafeLayout,
            edits,
        })
    }

    fn semantic(rule_id: &'static str, edits: Vec<Edit>) -> Option<Self> {
        (!edits.is_empty()).then_some(Self {
            rule_id,
            applicability: Applicability::SafeSemantic,
            edits,
        })
    }

    fn destructive(rule_id: &'static str, edits: Vec<Edit>) -> Option<Self> {
        (!edits.is_empty()).then_some(Self {
            rule_id,
            applicability: Applicability::UnsafeDestructive,
            edits,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Phase {
    Preprocessor,
    IncludeOrder,
    InvalidComments,
    UnusedVariables,
    EmptyStatements,
    CompactContinuations,
    BlankLines,
    BracesAndControls,
    CrowdedStatements,
    SingleStatementBlocks,
    RedundantElse,
    ForLoops,
    Ternaries,
    ChainedAssignments,
    SharedDeclarations,
    SplitDeclarations,
    FunctionLayout,
    Indentation,
    InitialDeclarations,
    TokenSpacing,
    Declarations,
    PointerNullReturns,
    CompactNullChecks,
    ReturnParentheses,
    DefinitionVoid,
    LongLines,
}

pub(crate) fn phases(options: &CActionOptions) -> Vec<Phase> {
    let mut result = vec![Phase::Preprocessor];
    if options.reorder_includes {
        result.push(Phase::IncludeOrder);
    }
    if options.remove_invalid_comments {
        result.push(Phase::InvalidComments);
    }
    if options.remove_unused_variables {
        result.push(Phase::UnusedVariables);
    }
    result.extend([
        Phase::EmptyStatements,
        Phase::CompactContinuations,
        Phase::BlankLines,
        Phase::BracesAndControls,
        Phase::CrowdedStatements,
        Phase::SingleStatementBlocks,
        Phase::RedundantElse,
        Phase::ForLoops,
        Phase::Ternaries,
        Phase::ChainedAssignments,
        Phase::SharedDeclarations,
        Phase::SplitDeclarations,
        Phase::FunctionLayout,
        Phase::Indentation,
        Phase::InitialDeclarations,
        Phase::TokenSpacing,
        Phase::Declarations,
        Phase::PointerNullReturns,
        Phase::CompactNullChecks,
        Phase::ReturnParentheses,
        Phase::DefinitionVoid,
        Phase::LongLines,
    ]);
    result
}

impl Phase {
    pub(crate) const fn one_shot(self) -> bool {
        // A loop nested in another is only reachable once the outer one has
        // become a `while`, so this phase has to be allowed to come back.
        !matches!(
            self,
            Self::CompactContinuations | Self::LongLines | Self::ForLoops | Self::ChainedAssignments
        )
    }

    // One arm per phase: the dispatch is a table, and splitting it would hide
    // the order the phases run in, which is what makes them compose.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn plan(
        self,
        context: &ParsedContext,
        diagnostics: &[ReportedDiagnostic],
        options: &CActionOptions,
    ) -> Result<Option<ActionBatch>, CActionError> {
        match self {
            Self::Preprocessor => Ok(ActionBatch::layout(
                "PREPROCESSOR_SPACING",
                format_preprocessors(context)?,
            )),
            Self::IncludeOrder => Ok(ActionBatch::semantic(
                "INCLUDE_ORDER",
                reorder_includes(context)?,
            )),
            Self::InvalidComments => Ok(ActionBatch::destructive(
                "REMOVE_INVALID_COMMENT",
                remove_invalid_comments(context, diagnostics)?,
            )),
            Self::EmptyStatements => Ok(ActionBatch::semantic(
                "REMOVE_EMPTY_STATEMENT",
                remove_empty_statements(context)?,
            )),
            // Proven by the fact, permitted by the option: the applicability
            // says how it is known to be safe, and `--unsafe` says whether the
            // reader asked for it at all.
            Self::UnusedVariables => Ok(ActionBatch::semantic(
                "REMOVE_UNUSED_VARIABLE",
                remove_unused_variables(context)?,
            )),
            Self::CompactContinuations => Ok(ActionBatch::layout(
                "COMPACT_CONTINUATION",
                compact_continuations(context, options.max_columns)?,
            )),
            Self::BlankLines => Ok(ActionBatch::layout(
                "BLANK_LINE_LAYOUT",
                fix_blank_lines(context, diagnostics)?,
            )),
            Self::BracesAndControls => Ok(ActionBatch::layout(
                "BRACE_CONTROL_LAYOUT",
                fix_braces_and_controls(context, diagnostics)?,
            )),
            Self::CrowdedStatements => Ok(ActionBatch::layout(
                "ONE_INSTRUCTION_PER_LINE",
                separate_crowded_statements(context)?,
            )),
            Self::SingleStatementBlocks => Ok(ActionBatch::semantic(
                "REMOVE_SINGLE_STATEMENT_BRACES",
                remove_single_statement_braces(context)?,
            )),
            Self::ForLoops => Ok(ActionBatch::semantic(
                "REPLACE_FOR_LOOP",
                rewrite_for_loops(context)?,
            )),
            Self::Ternaries => Ok(ActionBatch::semantic(
                "REPLACE_TERNARY",
                rewrite_ternaries(context)?,
            )),
            Self::ChainedAssignments => Ok(ActionBatch::semantic(
                "SPLIT_CHAINED_ASSIGNMENT",
                split_chained_assignments(context)?,
            )),
            Self::SharedDeclarations => Ok(ActionBatch::semantic(
                "ONE_DECLARATION_PER_LINE",
                split_shared_declarations(context)?,
            )),
            Self::SplitDeclarations => Ok(ActionBatch::semantic(
                "SPLIT_DECLARATION_ASSIGNMENT",
                split_declarations(context)?,
            )),
            Self::RedundantElse => Ok(ActionBatch::semantic(
                "REMOVE_REDUNDANT_ELSE",
                remove_redundant_else(context)?,
            )),
            Self::FunctionLayout => Ok(ActionBatch::layout(
                "FUNCTION_LAYOUT",
                fix_function_layout(context, diagnostics, options)?,
            )),
            Self::Indentation => Ok(ActionBatch::layout(
                "INDENTATION",
                fix_indentation(context, diagnostics)?,
            )),
            Self::InitialDeclarations => Ok(ActionBatch::layout(
                "INITIAL_DECLARATION_LAYOUT",
                format_initial_declarations(context)?,
            )),
            Self::TokenSpacing => Ok(ActionBatch::layout(
                "TOKEN_SPACING",
                fix_token_spacing(context, diagnostics)?,
            )),
            Self::Declarations => Ok(ActionBatch::layout(
                "DECLARATION_ALIGNMENT",
                align_declarations(context, diagnostics, options)?,
            )),
            Self::PointerNullReturns => Ok(ActionBatch::semantic(
                "POINTER_NULL_RETURN",
                replace_pointer_zero_returns(context)?,
            )),
            Self::CompactNullChecks => {
                if options.compact_null_checks {
                    Ok(ActionBatch::semantic(
                        "COMPACT_NULL_CHECK",
                        compact_null_checks(context)?,
                    ))
                } else {
                    Ok(None)
                }
            }
            Self::ReturnParentheses => Ok(ActionBatch::semantic(
                "RETURN_PARENTHESIS",
                parenthesize_returns(context, diagnostics)?,
            )),
            Self::DefinitionVoid => Ok(ActionBatch::semantic(
                "NO_ARGS_VOID",
                add_void_to_definitions(context, diagnostics)?,
            )),
            Self::LongLines => Ok(ActionBatch::layout(
                "LINE_TOO_LONG",
                wrap_long_lines(context, options.max_columns)?,
            )),
        }
    }
}

/// Reorders each contiguous include block: system headers first, then project
/// headers, alphabetically inside both categories.
///
/// A block ends at the first line that is not exactly one include directive, so
/// a comment, blank line, conditional, macro definition, or trailing text keeps
/// the surrounding directives where they are. That containment is the proof:
/// nothing is moved across a construct that could change what a header means.
fn reorder_includes(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let source = context.source();
    let mut edits = Vec::new();
    let mut block = Vec::<(u32, PhysicalLine, IncludeOrderKey)>::new();
    for (number, line, text) in context.lines().iter() {
        if let Some(key) = include_order_key(text) {
            block.push((number, line, key));
            continue;
        }
        append_include_order_edits(source, &block, &mut edits)?;
        block.clear();
    }
    append_include_order_edits(source, &block, &mut edits)?;
    Ok(edits)
}

fn append_include_order_edits(
    source: &str,
    block: &[(u32, PhysicalLine, IncludeOrderKey)],
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    if block.len() < 2 || block.windows(2).all(|pair| pair[0].2 <= pair[1].2) {
        return Ok(());
    }
    let mut order = (0..block.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| block[*left].2.cmp(&block[*right].2));
    for (slot, origin) in order.into_iter().enumerate() {
        if slot == origin {
            continue;
        }
        let (number, target, _) = block[slot];
        let (_, moved, _) = block[origin];
        edits.push(Edit::new(
            target.start,
            target.content_end,
            &source[moved.start..moved.content_end],
            "INCLUDE_ORDER",
            "reordered the include block with system headers before project headers, \
             alphabetically inside each",
            Some(number),
        )?);
    }
    Ok(())
}

fn format_preprocessors(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let source = context.source();
    let lines = context.lines();
    let blocked = multiline_preprocessor_lines(source, &lines);
    let mut depth = 0_usize;
    let mut edits = Vec::new();
    for (line_number, line, text) in lines.iter() {
        let leading = leading_whitespace(text);
        if text.as_bytes().get(leading) != Some(&b'#') {
            continue;
        }
        let mut cursor = leading + 1;
        while matches!(text.as_bytes().get(cursor), Some(b' ' | b'\t')) {
            cursor += 1;
        }
        let word_start = cursor;
        while text
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            cursor += 1;
        }
        if word_start == cursor {
            continue;
        }
        let directive = text[word_start..cursor].to_ascii_lowercase();
        let closes = matches!(directive.as_str(), "elif" | "else" | "endif");
        let effective_depth = depth.saturating_sub(usize::from(closes));
        if !blocked.contains(&line_number) {
            let argument = text[cursor..].trim_matches([' ', '\t']);
            let mut replacement = String::from("#");
            replacement.push_str(&" ".repeat(effective_depth));
            replacement.push_str(&text[word_start..cursor]);
            if !argument.is_empty() {
                replacement.push(' ');
                replacement.push_str(argument);
            }
            if replacement != text {
                edits.push(Edit::new(
                    line.start,
                    line.content_end,
                    replacement,
                    "PREPROCESSOR_SPACING",
                    "normalized preprocessor indentation and spacing",
                    Some(line_number),
                )?);
            }
        }
        if matches!(directive.as_str(), "if" | "ifdef" | "ifndef") {
            depth = depth.saturating_add(1);
        } else if directive == "endif" {
            depth = depth.saturating_sub(1);
        }
    }
    Ok(edits)
}

fn preprocessor_line_set(source: &str, lines: &SourceLines<'_>) -> BTreeSet<u32> {
    let mut result = BTreeSet::new();
    let mut active = false;
    for (line_number, _, text) in lines.iter() {
        let starts = text.trim_start_matches([' ', '\t']).starts_with('#');
        let continues = has_sensitive_line_end(text);
        if active || starts {
            result.insert(line_number);
            active = continues;
        } else {
            active = false;
        }
    }
    let _ = source;
    result
}

fn has_sensitive_line_end(text: &str) -> bool {
    let stripped = text.trim_end_matches([' ', '\t']);
    stripped.ends_with('\\') || stripped.ends_with("??/")
}

#[derive(Clone, Copy, Debug)]
struct Comment {
    start: usize,
    end: usize,
    line: u32,
    visual_column: u32,
}

fn remove_invalid_comments(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
) -> Result<Vec<Edit>, CActionError> {
    let targets: BTreeMap<(u32, u32), &str> = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code.as_str(),
                "WRONG_SCOPE_COMMENT" | "COMMENT_ON_INSTR"
            )
        })
        .map(|diagnostic| {
            (
                (diagnostic.line, diagnostic.visual_column),
                diagnostic.code.as_str(),
            )
        })
        .collect();
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let functions = function_infos(context);
    let official_header_end = official_header_end(context.source());
    let lines = context.lines();
    let mut edits = Vec::new();
    for comment in scan_comments(context.source(), &lines) {
        let Some(code) = targets.get(&(comment.line, comment.visual_column)) else {
            continue;
        };
        if *code == "WRONG_SCOPE_COMMENT"
            && !functions
                .iter()
                .any(|function| function.contains(comment.line))
        {
            continue;
        }
        if official_header_end.is_some_and(|end| comment.start < end) {
            continue;
        }
        let (start, end, replacement) = comment_removal(context.source(), comment);
        edits.push(Edit::new(
            start,
            end,
            replacement,
            "REMOVE_INVALID_COMMENT",
            "removed a comment at the exact location rejected by Norminette",
            Some(comment.line),
        )?);
    }
    Ok(edits)
}

fn scan_comments(source: &str, lines: &SourceLines<'_>) -> Vec<Comment> {
    let bytes = source.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    let mut state = QuoteState::Code;
    while index < bytes.len() {
        let following = bytes.get(index + 1).copied();
        match state {
            QuoteState::Code => {
                if bytes[index] == b'"' {
                    state = QuoteState::String;
                } else if bytes[index] == b'\'' {
                    state = QuoteState::Character;
                } else if bytes[index] == b'/' && following == Some(b'/') {
                    let start = index;
                    index += 2;
                    loop {
                        while index < bytes.len() && bytes[index] != b'\n' {
                            index += 1;
                        }
                        if index >= bytes.len() || !escaped_physical_newline(bytes, index) {
                            break;
                        }
                        index += 1;
                    }
                    result.push(comment_at(start, index, lines));
                    continue;
                } else if bytes[index] == b'/' && following == Some(b'*') {
                    let start = index;
                    index += 2;
                    while index + 1 < bytes.len()
                        && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                    {
                        index += 1;
                    }
                    if index + 1 >= bytes.len() {
                        break;
                    }
                    index += 2;
                    result.push(comment_at(start, index, lines));
                    continue;
                }
            }
            QuoteState::String | QuoteState::Character => {
                if bytes[index] == b'\\' && following.is_some() {
                    index += 2;
                    continue;
                }
                if (state == QuoteState::String && bytes[index] == b'"')
                    || (state == QuoteState::Character && bytes[index] == b'\'')
                {
                    state = QuoteState::Code;
                }
            }
        }
        index += 1;
    }
    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QuoteState {
    Code,
    String,
    Character,
}

fn comment_at(start: usize, end: usize, lines: &SourceLines<'_>) -> Comment {
    let line_number = lines.line_number_at(start);
    let visual_column = lines
        .get(line_number)
        .map_or(1, |line| lines.visual_column(line, start));
    Comment {
        start,
        end,
        line: line_number,
        visual_column,
    }
}

fn official_header_end(source: &str) -> Option<usize> {
    const EDGE: &str =
        "/* ************************************************************************** */";
    let line_index = SourceLines::index(source);
    let lines = SourceLines::new(source, &line_index);
    if lines.len() < 11 {
        return None;
    }
    let first = lines.get(1)?;
    let last = lines.get(11)?;
    let block = &source[first.start..last.end];
    (lines.text(first) == EDGE
        && lines.text(last) == EDGE
        && block.contains(":::      ::::::::")
        && block.contains("By:")
        && block.contains("Created:")
        && block.contains("Updated:"))
    .then_some(last.end)
}

fn comment_removal(source: &str, comment: Comment) -> (usize, usize, String) {
    let line_start = source[..comment.start]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    let line_end = source[comment.end..]
        .find('\n')
        .map_or(source.len(), |position| comment.end + position);
    let before = &source[line_start..comment.start];
    let after = &source[comment.end..line_end];
    if before.trim_matches([' ', '\t']).is_empty() && after.trim_matches([' ', '\t']).is_empty() {
        return (
            line_start,
            line_end + usize::from(line_end < source.len()),
            String::new(),
        );
    }
    if after.trim_matches([' ', '\t']).is_empty() {
        let mut start = comment.start;
        while start > line_start && matches!(source.as_bytes()[start - 1], b' ' | b'\t') {
            start -= 1;
        }
        return (start, comment.end, String::new());
    }
    let mut start = comment.start;
    while start > line_start && matches!(source.as_bytes()[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    let mut end = comment.end;
    while end < line_end && matches!(source.as_bytes()[end], b' ' | b'\t') {
        end += 1;
    }
    let surrounding = [&source[start..comment.start], &source[comment.end..end]].concat();
    let left = (start > 0).then(|| source.as_bytes()[start - 1] as char);
    let right = source.as_bytes().get(end).copied().map(char::from);
    let replacement = if surrounding.contains('\t') {
        "\t"
    } else if left.is_some_and(|character| "([{".contains(character))
        || right.is_some_and(|character| ")]},;".contains(character))
    {
        ""
    } else {
        " "
    };
    (start, end, replacement.to_owned())
}

fn fix_blank_lines(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
) -> Result<Vec<Edit>, CActionError> {
    let source = context.source();
    let lines = context.lines();
    let blocked = preprocessor_line_set(source, &lines);
    let has_local_preprocessor = diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_str(),
            "PREPOC_ONLY_GLOBAL" | "PREPROC_GLOBAL"
        )
    });
    let mut edits = Vec::new();
    for diagnostic in diagnostics {
        let Some(line) = lines.get(diagnostic.line) else {
            continue;
        };
        if blocked.contains(&diagnostic.line) {
            continue;
        }
        match diagnostic.code.as_str() {
            "NEWLINE_PRECEDES_FUNC" | "NL_AFTER_VAR_DECL" | "NL_AFTER_PREPROC" => {
                if diagnostic.code == "NL_AFTER_PREPROC" && has_local_preprocessor {
                    continue;
                }
                let previous = diagnostic
                    .line
                    .checked_sub(1)
                    .and_then(|number| lines.get(number));
                if previous.is_some_and(|previous| !lines.text(previous).trim().is_empty()) {
                    edits.push(Edit::new(
                        line.start,
                        line.start,
                        "\n",
                        diagnostic.code.clone(),
                        match diagnostic.code.as_str() {
                            "NEWLINE_PRECEDES_FUNC" => "inserted a blank line before a function",
                            "NL_AFTER_VAR_DECL" => "inserted a blank line after declarations",
                            _ => "inserted a blank line after preprocessing directives",
                        },
                        Some(diagnostic.line),
                    )?);
                }
            }
            "EMPTY_LINE_FUNCTION" | "CONSECUTIVE_NEWLINES"
                if lines.text(line).trim().is_empty() =>
            {
                edits.push(Edit::new(
                    line.start,
                    line.end,
                    "",
                    diagnostic.code.clone(),
                    if diagnostic.code == "EMPTY_LINE_FUNCTION" {
                        "removed a forbidden blank line inside a function"
                    } else {
                        "removed a consecutive blank line"
                    },
                    Some(diagnostic.line),
                )?);
            }
            _ => {}
        }
    }
    Ok(edits)
}

fn fix_braces_and_controls(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    let blocked = preprocessor_line_set(context.source(), &lines);
    let lexical = context.lexical();
    let mut edits = format_control_layout(context)?;
    for diagnostic in diagnostics {
        if !matches!(
            diagnostic.code.as_str(),
            "BRACE_NEWLINE" | "BRACE_SHOULD_EOL" | "EXP_NEWLINE"
        ) || blocked.contains(&diagnostic.line)
        {
            continue;
        }
        let Some(line) = lines.get(diagnostic.line) else {
            continue;
        };
        let text = lines.text(line);
        let diagnostic_byte = lines.byte_for_visual_column(line, diagnostic.visual_column);
        let relative = diagnostic_byte.saturating_sub(line.start);
        let indent = &text[..leading_whitespace(text)];
        match diagnostic.code.as_str() {
            "BRACE_NEWLINE" => {
                let brace = find_brace_near(text, line.start, relative, lexical, None);
                let Some(brace) = brace else {
                    continue;
                };
                let (start, _) = whitespace_before(text, brace);
                edits.push(Edit::new(
                    line.start + start,
                    line.start + brace,
                    format!("\n{indent}"),
                    "BRACE_NEWLINE",
                    "placed the opening brace on its own line",
                    Some(diagnostic.line),
                )?);
            }
            "BRACE_SHOULD_EOL" => {
                let Some(brace) = find_brace_near(text, line.start, relative, lexical, Some("{}"))
                else {
                    continue;
                };
                let (start, end) = whitespace_after(text, brace + 1);
                if end >= text.len() {
                    continue;
                }
                let extra = if text.as_bytes()[brace] == b'{' {
                    "\t"
                } else {
                    ""
                };
                edits.push(Edit::new(
                    line.start + start,
                    line.start + end,
                    format!("\n{indent}{extra}"),
                    "BRACE_SHOULD_EOL",
                    "placed the brace on its own line",
                    Some(diagnostic.line),
                )?);
            }
            "EXP_NEWLINE" => {
                if let Some(close) = control_condition_close(text, line.start, lexical) {
                    let (start, end) = whitespace_after(text, close + 1);
                    // A body that opens with a brace belongs to the native
                    // brace rule, which puts it at the control's own indent.
                    // This arm would put it one tab deeper — wrong for a brace —
                    // and both edits land on the same byte, so the batch was
                    // rejected as conflicting and every other fix in the file
                    // was lost with it.
                    let opens_with_brace = text.as_bytes().get(end) == Some(&b'{');
                    if end < text.len() && !opens_with_brace {
                        edits.push(Edit::new(
                            line.start + start,
                            line.start + end,
                            format!("\n{indent}\t"),
                            "EXP_NEWLINE",
                            "moved the control body to the next line",
                            Some(diagnostic.line),
                        )?);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(edits)
}

fn format_control_layout(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    // Every rule below skips preprocessor lines, and each used to derive that
    // set for itself by scanning the whole file — once per rule, and once per
    // block for the one that runs per block. It is the same answer for all of
    // them, and on a large file deriving it repeatedly dominated this phase.
    let blocked = preprocessor_line_set(context.source(), &lines);
    let mut edits = Vec::new();
    for token in context.tokens().iter().filter(|token| token.text == "else") {
        let line_number = lines.line_number_at(token.start);
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let relative = token.start.saturating_sub(line.start);
        if !text[..relative].trim().is_empty() {
            let (start, _) = whitespace_before(text, relative);
            let indent = &text[..leading_whitespace(text)];
            edits.push(Edit::new(
                line.start + start,
                token.start,
                format!("\n{indent}"),
                "ELSE_NEWLINE",
                "placed else on its own line",
                Some(line_number),
            )?);
        }
    }
    for body in &context.facts().control_compounds {
        let brace = body.start().get() as usize;
        push_control_brace_newline(context, brace, &mut edits)?;
    }
    push_control_keyword_spacing(context, &blocked, &mut edits)?;
    push_binary_operator_spacing(context, &blocked, &mut edits)?;
    push_comma_spacing(context, &blocked, &mut edits)?;
    push_subscript_spacing(context, &blocked, &mut edits)?;
    push_pointer_spacing(context, &blocked, &mut edits)?;
    push_inline_body_newline(context, &blocked, &mut edits)?;
    push_block_edge_blank_lines(context, &mut edits)?;
    for body in &context.facts().compound_bodies {
        push_block_line_breaks(context, *body, &blocked, &mut edits)?;
    }
    Ok(edits)
}

/// Puts exactly one space between a control keyword and its parenthesis.
///
/// The keyword set is closed and reserved, so no call, macro, or identifier can
/// land here by accident. Being provable from the tokens alone is what makes it
/// worth having: it is the same answer with or without the official checker, so
/// the browser playground reaches it too.
fn push_control_keyword_spacing(
    context: &ParsedContext,
    blocked: &BTreeSet<u32>,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    const KEYWORDS: [&str; 5] = ["if", "while", "for", "switch", "return"];
    let lines = context.lines();
    for token in context
        .tokens()
        .iter()
        .filter(|token| KEYWORDS.contains(&token.text.as_str()))
    {
        let line_number = lines.line_number_at(token.start);
        if blocked.contains(&line_number) {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let relative = token.end.saturating_sub(line.start);
        let (start, end) = whitespace_after(text, relative);
        if text.as_bytes().get(end) != Some(&b'(') || &text[start..end] == " " {
            continue;
        }
        edits.push(Edit::new(
            line.start + start,
            line.start + end,
            " ",
            "CONTROL_KEYWORD_SPACING",
            "left exactly one space between a control keyword and its parenthesis",
            Some(line_number),
        )?);
    }
    Ok(())
}

fn push_control_brace_newline(
    context: &ParsedContext,
    brace: usize,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    let lines = context.lines();
    let line_number = lines.line_number_at(brace);
    let Some(line) = lines.get(line_number) else {
        return Ok(());
    };
    let text = lines.text(line);
    let relative = brace.saturating_sub(line.start);
    if relative >= text.len() || text[..relative].trim().is_empty() {
        return Ok(());
    }
    // A brace written as `){` has no whitespace to replace, so this used to
    // bail and leave the file with an official error after a run that reported
    // success. An empty range is a valid insertion point.
    let (start, _) = whitespace_before(text, relative);
    let indent = &text[..leading_whitespace(text)];
    edits.push(Edit::new(
        line.start + start,
        brace,
        format!("\n{indent}"),
        "CONTROL_BRACE_NEWLINE",
        "placed a control block opening brace on its own line",
        Some(line_number),
    )?);
    Ok(())
}

/// Gives a single-statement control body its own line.
///
/// The tree says where the body begins, so `if (n)n = 2;` is split without
/// scanning the text for the parenthesis that closed the condition.
fn push_inline_body_newline(
    context: &ParsedContext,
    blocked: &BTreeSet<u32>,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    let lines = context.lines();
    for body in &context.facts().control_inline_bodies {
        let start = body.start().get() as usize;
        let line_number = lines.line_number_at(start);
        if blocked.contains(&line_number) {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let relative = start.saturating_sub(line.start);
        if text[..relative].trim().is_empty() {
            continue;
        }
        let indent = text[..leading_whitespace(text)].to_owned();
        let (whitespace_start, _) = whitespace_before(text, relative);
        edits.push(Edit::new(
            line.start + whitespace_start,
            start,
            format!("\n{indent}\t"),
            "CONTROL_BODY_NEWLINE",
            "placed a control body on its own line",
            Some(line_number),
        )?);
    }
    Ok(())
}

/// Removes a blank line pressed against a block's opening or closing brace.
///
/// A blank line means a separation between two things, and there is nothing on
/// the other side of a brace to separate from.
fn push_block_edge_blank_lines(
    context: &ParsedContext,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    let source = context.source();
    let lines = context.lines();
    for body in &context.facts().compound_bodies {
        let open_line = lines.line_number_at(body.start().get() as usize);
        let close_line = lines.line_number_at((body.end().get() as usize).saturating_sub(1));
        if close_line <= open_line + 1 {
            continue;
        }
        for (line_number, keep) in [(open_line + 1, open_line), (close_line - 1, close_line)] {
            let Some(line) = lines.get(line_number) else {
                continue;
            };
            if !lines.text(line).trim().is_empty() {
                continue;
            }
            let Some(anchor) = lines.get(keep) else {
                continue;
            };
            let (start, end) = if keep < line_number {
                (anchor.end, line.end)
            } else {
                (line.start, anchor.start)
            };
            if start < end && source.get(start..end).is_some() {
                edits.push(Edit::new(
                    start,
                    end,
                    "",
                    "BLOCK_EDGE_BLANK_LINE",
                    "removed a blank line pressed against a brace",
                    Some(line_number),
                )?);
            }
        }
    }
    Ok(())
}

/// Binds a declarator star to the name that follows it.
///
/// The grammar separates this star from multiplication, which is what makes
/// the edit safe: `char * text` becomes `char *text`, while `a * b` is a
/// binary expression and is never reached from here.
fn push_pointer_spacing(
    context: &ParsedContext,
    blocked: &BTreeSet<u32>,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    let lines = context.lines();
    for star in &context.facts().pointer_stars {
        let end = star.end().get() as usize;
        let line_number = lines.line_number_at(end);
        if blocked.contains(&line_number) {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let (start, stop) = whitespace_after(text, end.saturating_sub(line.start));
        if start == stop || stop >= text.len() {
            continue;
        }
        edits.push(Edit::new(
            line.start + start,
            line.start + stop,
            "",
            "POINTER_SPACING",
            "bound a declarator star to the name after it",
            Some(line_number),
        )?);
    }
    Ok(())
}

/// Removes padding inside a subscript.
///
/// `numbers[ 2 ]` is the index written with room around it; the brackets are
/// punctuation, so the space has nothing to separate.
fn push_subscript_spacing(
    context: &ParsedContext,
    blocked: &BTreeSet<u32>,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    let lines = context.lines();
    let lexical = context.lexical();
    for token in context
        .tokens()
        .iter()
        .filter(|token| token.text == "[" || token.text == "]")
    {
        let line_number = lines.line_number_at(token.start);
        if blocked.contains(&line_number) || lexical.is_protected(token.start) {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let relative = token.start.saturating_sub(line.start);
        let (start, end) = if token.text == "[" {
            whitespace_after(text, relative + 1)
        } else {
            whitespace_before(text, relative)
        };
        // A bracket at the edge of its line is a line break, not padding.
        if start == end || end >= text.len() || start == 0 {
            continue;
        }
        edits.push(Edit::new(
            line.start + start,
            line.start + end,
            "",
            "SUBSCRIPT_SPACING",
            "removed padding inside a subscript",
            Some(line_number),
        )?);
    }
    Ok(())
}

/// Puts no space before a comma and one space after it.
///
/// A comma separates, so it never has an operand of its own to disambiguate:
/// the token alone settles it, as long as it is real punctuation rather than a
/// byte inside a string or a comment.
fn push_comma_spacing(
    context: &ParsedContext,
    blocked: &BTreeSet<u32>,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    let lines = context.lines();
    let lexical = context.lexical();
    for token in context.tokens().iter().filter(|token| token.text == ",") {
        let line_number = lines.line_number_at(token.start);
        if blocked.contains(&line_number) || lexical.is_protected(token.start) {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let relative = token.start.saturating_sub(line.start);
        let (before_start, _) = whitespace_before(text, relative);
        if before_start < relative && before_start > 0 {
            edits.push(Edit::new(
                line.start + before_start,
                token.start,
                "",
                "COMMA_SPACING",
                "removed the space before a comma",
                Some(line_number),
            )?);
        }
        // A comma that ends its line already separates by the line break.
        let (after_start, after_end) = whitespace_after(text, relative + 1);
        if after_end < text.len() && &text[after_start..after_end] != " " {
            edits.push(Edit::new(
                line.start + after_start,
                line.start + after_end,
                " ",
                "COMMA_SPACING",
                "left exactly one space after a comma",
                Some(line_number),
            )?);
        }
    }
    Ok(())
}

/// Puts exactly one space on each side of a binary or assignment operator.
///
/// The proof is the node kind: the grammar gives `-a` and `a++` their own
/// kinds, so an operator reached this way always has an operand on each side.
/// Reacting to a one-sided official diagnostic instead used to leave `sum -1`,
/// which the checker accepts and a reader reads as a unary minus.
fn push_binary_operator_spacing(
    context: &ParsedContext,
    blocked: &BTreeSet<u32>,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    let lines = context.lines();
    for operator in &context.facts().binary_operators {
        let start = operator.start().get() as usize;
        let end = operator.end().get() as usize;
        let line_number = lines.line_number_at(start);
        if blocked.contains(&line_number) || lines.line_number_at(end) != line_number {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let (open, close) = (
            start.saturating_sub(line.start),
            end.saturating_sub(line.start),
        );

        // A line break on either side is a deliberate continuation, and
        // collapsing it would be a different edit than adding a space.
        let (before_start, _) = whitespace_before(text, open);
        if before_start > 0 && &text[before_start..open] != " " {
            edits.push(Edit::new(
                line.start + before_start,
                start,
                " ",
                "OPERATOR_SPACING",
                "left exactly one space before a binary operator",
                Some(line_number),
            )?);
        }
        let (after_start, after_end) = whitespace_after(text, close);
        if after_end < text.len() && &text[after_start..after_end] != " " {
            edits.push(Edit::new(
                line.start + after_start,
                line.start + after_end,
                " ",
                "OPERATOR_SPACING",
                "left exactly one space after a binary operator",
                Some(line_number),
            )?);
        }
    }
    Ok(())
}

/// Gives a block's contents and its closing brace their own lines.
///
/// This runs in the same pass as the edit that moves the opening brace, so it
/// reads the indentation of the line the brace will end up on rather than the
/// brace's current position. The phase gets exactly one turn, and a rule that
/// waited for the next one would simply never fire.
fn push_block_line_breaks(
    context: &ParsedContext,
    body: TextRange,
    blocked: &BTreeSet<u32>,
    edits: &mut Vec<Edit>,
) -> Result<(), CActionError> {
    let lines = context.lines();
    let open = body.start().get() as usize;
    let close = (body.end().get() as usize).saturating_sub(1);
    let open_line_number = lines.line_number_at(open);
    let close_line_number = lines.line_number_at(close);
    if blocked.contains(&open_line_number) || blocked.contains(&close_line_number) {
        return Ok(());
    }
    let Some(open_line) = lines.get(open_line_number) else {
        return Ok(());
    };
    let open_text = lines.text(open_line);
    let open_relative = open.saturating_sub(open_line.start);
    let indent = open_text[..leading_whitespace(open_text)].to_owned();

    // Whatever follows the opening brace on its line.
    let (after_start, after_end) = whitespace_after(open_text, open_relative + 1);
    if after_end < open_text.len() && close > open_line.start + after_end {
        edits.push(Edit::new(
            open_line.start + after_start,
            open_line.start + after_end,
            format!("\n{indent}\t"),
            "BLOCK_STATEMENT_NEWLINE",
            "placed a block statement on its own line",
            Some(open_line_number),
        )?);
    }

    // The closing brace, when something shares its line.
    let Some(close_line) = lines.get(close_line_number) else {
        return Ok(());
    };
    let close_text = lines.text(close_line);
    let close_relative = close.saturating_sub(close_line.start);
    if close_relative == 0 || close_text[..close_relative].trim().is_empty() {
        return Ok(());
    }
    let (start, _) = whitespace_before(close_text, close_relative);
    edits.push(Edit::new(
        close_line.start + start,
        close,
        format!("\n{indent}"),
        "BLOCK_BRACE_NEWLINE",
        "placed a closing brace on its own line",
        Some(close_line_number),
    )?);
    Ok(())
}

fn remove_single_statement_braces(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let source = context.source();
    let lines = context.lines();
    let mut edits = Vec::new();
    for body in &context.facts().single_statement_bodies {
        let start = body.compound_range.start().get() as usize;
        let end = body.compound_range.end().get() as usize;
        let statement_start = body.statement_range.start().get() as usize;
        let statement_end = body.statement_range.end().get() as usize;
        let brace_line = lines.line_number_at(start);
        let statement_line = lines.line_number_at(statement_start);
        let Some(line) = lines.get(brace_line) else {
            continue;
        };
        if !lines.text(line)[..start.saturating_sub(line.start)]
            .trim()
            .is_empty()
            || statement_line == brace_line
        {
            continue;
        }
        let Some(statement) = source.get(statement_start..statement_end) else {
            continue;
        };
        edits.push(Edit::new(
            start,
            end,
            format!("\t{statement}"),
            "REMOVE_SINGLE_STATEMENT_BRACES",
            "removed a scope-free single-statement control block",
            Some(brace_line),
        )?);
    }
    Ok(edits)
}

/// Deletes a statement that is nothing but `;`.
///
/// The edit reaches back to the last character that is not whitespace, so a
/// `;` alone on its own line takes its line with it instead of leaving a blank
/// one behind, while a second `;` glued to the end of a statement removes only
/// itself.
fn remove_empty_statements(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let source = context.source();
    let lines = context.lines();
    let mut edits = Vec::new();
    for range in &context.facts().empty_statements {
        let end = range.end().get() as usize;
        let start = source[..range.start().get() as usize]
            .trim_end_matches(|character: char| character.is_whitespace())
            .len();
        edits.push(Edit::new(
            start,
            end,
            "",
            "REMOVE_EMPTY_STATEMENT",
            "removed a statement that was only a semicolon",
            Some(lines.line_number_at(end.saturating_sub(1))),
        )?);
    }
    Ok(edits)
}

/// Deletes a local nothing reads, when nothing in it runs.
///
/// The proof is the fact's, and it is deliberately not the compiler's. The
/// compiler runs after this stage precisely so its findings never authorize an
/// edit, and `-Wunused-variable` would be the wrong permission anyway: it fires
/// for `int n = g();` exactly as it does for `int n;`, and deleting the first
/// deletes a call. A declaration holding a `malloc` is the sharpest case —
/// removing it would repair a leak by accident, into a program the reader did
/// not write. Proving it here instead means the browser reaches it too.
fn remove_unused_variables(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    let mut edits = Vec::new();
    for declaration in &context.facts().inert_declarations {
        let start = declaration.range.start().get() as usize;
        let end = declaration.range.end().get() as usize;
        let line_number = lines.line_number_at(start);
        if lines.line_number_at(end) != line_number {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        // Only when the declaration is the whole line. Anything sharing it is
        // something this rule was never shown.
        let text = lines.text(line);
        if text.trim() != context.source().get(start..end).unwrap_or_default().trim() {
            continue;
        }
        edits.push(Edit::new(
            line.start,
            line.end,
            "",
            "REMOVE_UNUSED_VARIABLE",
            "removed a variable the compiler proved unused",
            Some(line_number),
        )?);
    }
    Ok(edits)
}

/// Separates a declaration from the value it was given on the same line.
///
/// `int teste = 10;` becomes `int teste;` plus `teste = 10;` at the top of the
/// instructions, which is what the official checker asks for when it reports
/// `DECL_ASSIGN_LINE`. The assignments keep their declaration order, so one
/// initializer that reads a variable declared above it still reads the value
/// that was just assigned.
///
/// All of a block's assignments are inserted as one edit. Emitting them
/// separately would put several edits at the same byte, and the batch would be
/// rejected as conflicting — taking every other fix in the file down with it.
fn split_declarations(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    use std::fmt::Write as _;

    let source = context.source();
    let lines = context.lines();
    let facts = context.facts();
    let mut edits = Vec::new();
    for block in &facts.initial_declaration_blocks {
        let Some(following) = block.following_item else {
            continue;
        };
        let line_number = lines.line_number_at(following.start().get() as usize);
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        // The assignments go above the first instruction's whole line, not at
        // the byte where its text begins, or they land after its indentation
        // and push it to column zero.
        let insertion = line.start;
        let indent = lines.text(line)[..leading_whitespace(lines.text(line))].to_owned();
        let mut assignments = String::new();
        for declaration in &block.declarations {
            let Some(split) = facts
                .declaration_splits
                .iter()
                .find(|split| split.declaration_range == *declaration)
            else {
                continue;
            };
            let start = split.strip_range.start().get() as usize;
            let end = split.strip_range.end().get() as usize;
            let value_start = split.value_range.start().get() as usize;
            let value_end = split.value_range.end().get() as usize;
            let (Some(value), true) = (source.get(value_start..value_end), start < end) else {
                continue;
            };
            edits.push(Edit::new(
                start,
                end,
                "",
                "SPLIT_DECLARATION_ASSIGNMENT",
                "separated a declaration from its value",
                Some(lines.line_number_at(start)),
            )?);
            let _ = writeln!(assignments, "{indent}{} = {value};", split.name);
        }
        if !assignments.is_empty() {
            edits.push(Edit::new(
                insertion,
                insertion,
                assignments,
                "SPLIT_DECLARATION_ASSIGNMENT",
                "moved the value below the declarations",
                Some(line_number),
            )?);
        }
    }
    Ok(edits)
}

/// The Norm's limit on a function body, which a rewrite that adds lines spends.
const FUNCTION_LINES: u32 = 25;

/// Writes a chained assignment as the two it stands for.
///
/// `a = b = 0;` becomes `b = 0;` and then `a = b;`, in that order. The second
/// reads `b` rather than repeating the value, because that is what the chain
/// did: `a` takes what `b` holds after any conversion `b`'s type imposes. A
/// call on the right still runs exactly once, in the first statement.
fn split_chained_assignments(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    let mut edits = Vec::new();
    let mut spent: HashMap<TextRange, u32> = HashMap::new();
    for chain in &context.facts().chained_assignments {
        let start = chain.statement_range.start().get() as usize;
        let end = chain.statement_range.end().get() as usize;
        let line_number = lines.line_number_at(start);
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let already = spent.entry(chain.function_body_range).or_default();
        if body_line_count(&lines, chain.function_body_range) + *already + 1 > FUNCTION_LINES {
            continue;
        }
        let text = lines.text(line);
        let indent = &text[..leading_whitespace(text)];
        edits.push(Edit::new(
            start,
            end,
            format!(
                "{};\n{indent}{} {} {};",
                chain.inner, chain.target, chain.operator, chain.inner_target
            ),
            "SPLIT_CHAINED_ASSIGNMENT",
            "wrote a chained assignment as the two it stood for",
            Some(line_number),
        )?);
        *already += 1;
    }
    Ok(edits)
}

/// Gives each variable of a shared declaration its own line.
///
/// `int *a, b;` declares one pointer and one int, which is the reason the Norm
/// asks for one per line: written out, nobody has to remember that the star
/// binds to the name. Each declarator is copied exactly as written, so the
/// pointer stays a pointer and an array keeps its bound, and the specifiers in
/// front are repeated verbatim rather than rebuilt from a type this would have
/// to guess at.
fn split_shared_declarations(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    use std::fmt::Write as _;

    let lines = context.lines();
    let mut edits = Vec::new();
    for declaration in &context.facts().shared_declarations {
        let start = declaration.range.start().get() as usize;
        let end = declaration.range.end().get() as usize;
        let line_number = lines.line_number_at(start);
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        // Anything sharing the line, before or after, means the replacement
        // would land beside text it knows nothing about.
        let text = lines.text(line);
        let relative = start.saturating_sub(line.start);
        if !text[..relative].trim().is_empty() || !text[end.saturating_sub(line.start)..].trim().is_empty()
        {
            continue;
        }
        let indent = &text[..leading_whitespace(text)];
        let mut replacement = String::new();
        for (index, declarator) in declaration.declarators.iter().enumerate() {
            if index > 0 {
                let _ = write!(replacement, "\n{indent}");
            }
            // A tab, never a space: the Norm puts one between a type and the
            // name it declares, and the rule that lines the names up expects
            // to find one there.
            let _ = write!(replacement, "{}\t{declarator};", declaration.specifiers);
        }
        edits.push(Edit::new(
            start,
            end,
            replacement,
            "ONE_DECLARATION_PER_LINE",
            "gave each declared name its own line",
            Some(line_number),
        )?);
    }
    Ok(edits)
}

/// Gives a second instruction sharing a line its own line.
///
/// The Norm allows one instruction or control structure per line. Nothing here
/// moves a token or changes one: the whitespace between two statements becomes
/// a newline and the indentation the first one already had, which is why this
/// keeps the layout fingerprint rather than needing a semantic proof.
fn separate_crowded_statements(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    let mut edits = Vec::new();
    for gap in &context.facts().crowded_statements {
        let start = gap.start().get() as usize;
        let end = gap.end().get() as usize;
        let line_number = lines.line_number_at(start);
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let indent = &text[..leading_whitespace(text)];
        edits.push(Edit::new(
            start,
            end,
            format!("\n{indent}"),
            "ONE_INSTRUCTION_PER_LINE",
            "gave a second instruction on the line its own line",
            Some(line_number),
        )?);
    }
    Ok(edits)
}

/// Replaces a forbidden `for` with the `while` that says the same loop.
///
/// The initializer runs once, so it moves above the loop. The condition is the
/// `while` condition, and an absent one is the `1` the `for` meant. The step
/// goes last in the body, where it still runs after every iteration — which is
/// exactly what the fact's guards establish, since a `continue` would reach it
/// in a `for` and skip it here.
///
/// A body that already has braces keeps them and its own indentation, and the
/// step is spliced in before the closing one. A body that is a single statement
/// gains braces, because it is about to hold two statements.
fn rewrite_for_loops(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    use std::fmt::Write as _;

    let source = context.source();
    let lines = context.lines();
    let mut edits = Vec::new();
    let mut spent: HashMap<TextRange, u32> = HashMap::new();
    // Facts arrive outermost first, and one loop nested in another gives two
    // edits over the same bytes, which would take the whole batch down. The
    // outer one goes now; the inner one is still a loop in a block afterwards,
    // so the next pass reaches it.
    let mut rewritten_through = 0_usize;
    for loop_fact in &context.facts().for_loops {
        let start = loop_fact.statement_range.start().get() as usize;
        let end = loop_fact.statement_range.end().get() as usize;
        if start < rewritten_through {
            continue;
        }
        let text = |range: Option<TextRange>| range.and_then(|range| source.get(range_bounds(range)));
        let (initializer, step) = (text(loop_fact.initializer_range), text(loop_fact.step_range));
        let condition = text(loop_fact.condition_range).unwrap_or("1");
        let Some(body) = source.get(range_bounds(loop_fact.body_range)) else {
            continue;
        };
        let line_number = lines.line_number_at(start);
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        // A comment written after the loop's last statement is a sibling of the
        // loop, not part of it, so replacing the loop would leave it stranded
        // below the closing brace, describing the wrong thing. The reader's
        // words stay where the reader put them.
        let closing = lines.line_number_at(end.saturating_sub(1));
        if lines
            .get(closing)
            .is_some_and(|last| !lines.text(last)[end.saturating_sub(last.start)..].trim().is_empty())
        {
            continue;
        }
        let indent = lines.text(line)[..leading_whitespace(lines.text(line))].to_owned();
        let growth = u32::from(initializer.is_some())
            + u32::from(step.is_some())
            + if loop_fact.body_is_block { 0 } else { 2 };
        let already = spent.entry(loop_fact.function_body_range).or_default();
        if body_line_count(&lines, loop_fact.function_body_range) + *already + growth
            > FUNCTION_LINES
        {
            continue;
        }
        let mut replacement = String::new();
        if let Some(initializer) = initializer {
            let _ = write!(replacement, "{initializer};\n{indent}");
        }
        let _ = write!(replacement, "while ({condition})\n{indent}");
        match (loop_fact.body_is_block, step) {
            (true, None) => replacement.push_str(body),
            // Everything up to the closing brace already ends with that
            // brace's own indentation, so the step lands beside the statements
            // it follows rather than beside the brace.
            (true, Some(step)) => {
                let Some(inner) = body.strip_suffix('}') else {
                    continue;
                };
                let _ = write!(replacement, "{inner}\t{step};\n{indent}}}");
            }
            // Braces are for a body that holds more than one instruction, and
            // the rule that takes needless ones away has already had its turn
            // by the time this phase runs. Emitting them only where they are
            // needed leaves nothing behind for it to undo.
            (false, None) => {
                let _ = write!(replacement, "\t{body}");
            }
            // A body of only `;` did nothing but hold the loop open for the
            // step. Written out, the step is the whole body.
            (false, Some(step)) if body.trim() == ";" => {
                let _ = write!(replacement, "\t{step};");
            }
            (false, Some(step)) => {
                let _ = write!(
                    replacement,
                    "{{\n{indent}\t{body}\n{indent}\t{step};\n{indent}}}"
                );
            }
        }
        edits.push(Edit::new(
            start,
            end,
            replacement,
            "REPLACE_FOR_LOOP",
            "replaced a forbidden for with the while it stood for",
            Some(line_number),
        )?);
        *already += growth;
        rewritten_through = end;
    }
    Ok(edits)
}

/// Replaces a forbidden `?:` with the branch it was hiding.
///
/// `x = a > b ? a : b;` becomes an `if`/`else` writing to `x`, and
/// `return (c ? a : b);` becomes an `if` that returns, then a return. Both keep
/// the original evaluation order exactly: the condition runs first, then one
/// branch and never the other, which is what the operator did.
///
/// One line becomes three or four, and a function is allowed twenty-five. A
/// rewrite that pushed a function over that limit would trade a ternary the
/// student can rewrite in place for a structural error that forces them to
/// carve up a function, so the count is checked first and the statement is left
/// to be reported when the room is not there.
fn rewrite_ternaries(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    use std::fmt::Write as _;


    let source = context.source();
    let lines = context.lines();
    let mut edits = Vec::new();
    let mut spent: HashMap<TextRange, u32> = HashMap::new();
    for ternary in &context.facts().ternary_statements {
        let start = ternary.statement_range.start().get() as usize;
        let end = ternary.statement_range.end().get() as usize;
        let (Some(condition), Some(consequence), Some(alternative)) = (
            source.get(range_bounds(ternary.condition_range)),
            source.get(range_bounds(ternary.consequence_range)),
            source.get(range_bounds(ternary.alternative_range)),
        ) else {
            continue;
        };
        let line_number = lines.line_number_at(start);
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        // The replacement text starts where the statement's first token does,
        // so its own indentation is already on the line; every line after it
        // has to carry that indentation itself.
        let indent = lines.text(line)[..leading_whitespace(lines.text(line))].to_owned();
        let growth = match ternary.form {
            TernaryForm::Return => 2,
            TernaryForm::Assignment { .. } => 3,
        };
        let already = spent.entry(ternary.function_body_range).or_default();
        if body_line_count(&lines, ternary.function_body_range) + *already + growth > FUNCTION_LINES
        {
            continue;
        }
        let test = if ternary.condition_parenthesized {
            format!("if {condition}")
        } else {
            format!("if ({condition})")
        };
        let mut replacement = String::new();
        match &ternary.form {
            TernaryForm::Return => {
                let _ = write!(
                    replacement,
                    "{test}\n{indent}\treturn {};\n{indent}return {};",
                    parenthesized(consequence),
                    parenthesized(alternative),
                );
            }
            TernaryForm::Assignment { target, operator } => {
                let _ = write!(
                    replacement,
                    "{test}\n{indent}\t{target} {operator} {consequence};\n\
                     {indent}else\n{indent}\t{target} {operator} {alternative};",
                );
            }
        }
        edits.push(Edit::new(
            start,
            end,
            replacement,
            "REPLACE_TERNARY",
            "replaced a forbidden ternary with the branch it stood for",
            Some(line_number),
        )?);
        *already += growth;
    }
    Ok(edits)
}

/// A returned value, wearing the parentheses the Norm asks of it exactly once.
fn parenthesized(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        trimmed.to_owned()
    } else {
        format!("({trimmed})")
    }
}

/// Lines a function body occupies, counted as the official checker counts
/// them: the function's own braces are not part of its twenty-five.
fn body_line_count(lines: &SourceLines, body: TextRange) -> u32 {
    let opening = lines.line_number_at(body.start().get() as usize);
    let closing = lines.line_number_at((body.end().get() as usize).saturating_sub(1));
    closing.saturating_sub(opening).saturating_sub(1)
}

fn range_bounds(range: TextRange) -> std::ops::Range<usize> {
    range.start().get() as usize..range.end().get() as usize
}

fn remove_redundant_else(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let source = context.source();
    let lines = context.lines();
    let mut edits = Vec::new();
    for branch in &context.facts().redundant_else_branches {
        let start = branch.else_keyword_range.start().get() as usize;
        let end = branch.alternative_range.end().get() as usize;
        let return_start = branch.return_range.start().get() as usize;
        let return_end = branch.return_range.end().get() as usize;
        let line_number = lines.line_number_at(start);
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let relative = start.saturating_sub(line.start);
        if !lines.text(line)[..relative].trim().is_empty() {
            continue;
        }
        let Some(return_statement) = source.get(return_start..return_end) else {
            continue;
        };
        edits.push(Edit::new(
            start,
            end,
            return_statement,
            "REMOVE_REDUNDANT_ELSE",
            "removed else after an unconditional return",
            Some(line_number),
        )?);
    }
    Ok(edits)
}

fn find_brace_near(
    text: &str,
    base: usize,
    relative: usize,
    lexical: &LexicalMap,
    allowed: Option<&str>,
) -> Option<usize> {
    let accepted = |byte: u8| match allowed {
        Some(set) => set.as_bytes().contains(&byte),
        None => byte == b'{',
    };
    let lower = relative.saturating_sub(2);
    let upper = (relative + 3).min(text.len());
    (lower..upper)
        .find(|index| accepted(text.as_bytes()[*index]) && !lexical.is_protected(base + *index))
        .or_else(|| {
            (0..text.len()).rev().find(|index| {
                accepted(text.as_bytes()[*index]) && !lexical.is_protected(base + *index)
            })
        })
}

fn control_condition_close(text: &str, base: usize, lexical: &LexicalMap) -> Option<usize> {
    for keyword in ["if", "while", "for", "switch"] {
        let mut search = 0;
        while let Some(found) = text[search..].find(keyword) {
            let start = search + found;
            let end = start + keyword.len();
            let left_ok = start == 0
                || !text.as_bytes()[start - 1].is_ascii_alphanumeric()
                    && text.as_bytes()[start - 1] != b'_';
            let right_ok = end == text.len()
                || !text.as_bytes()[end].is_ascii_alphanumeric() && text.as_bytes()[end] != b'_';
            if left_ok && right_ok && !lexical.is_protected(base + start) {
                let mut opening = end;
                while matches!(text.as_bytes().get(opening), Some(b' ' | b'\t')) {
                    opening += 1;
                }
                if text.as_bytes().get(opening) == Some(&b'(') {
                    let mut depth = 0_u32;
                    for index in opening..text.len() {
                        if lexical.is_protected(base + index) {
                            continue;
                        }
                        match text.as_bytes()[index] {
                            b'(' => depth = depth.saturating_add(1),
                            b')' => {
                                depth = depth.checked_sub(1)?;
                                if depth == 0 {
                                    return Some(index);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            search = end;
        }
    }
    None
}

#[derive(Clone, Debug)]
struct ContinuationLine {
    number: u32,
    start: usize,
    text: String,
    indent_end: usize,
    has_comment: bool,
    preprocessor: bool,
    splice_barrier: bool,
    delimiter_depth_after: u32,
}

fn compact_continuations(
    context: &ParsedContext,
    max_columns: u32,
) -> Result<Vec<Edit>, CActionError> {
    if max_columns == 0 || !context.source().contains('\n') {
        return Ok(Vec::new());
    }
    let scanned = continuation_lines(context);
    if scanned.len() < 2 {
        return Ok(Vec::new());
    }
    let mut edits = Vec::new();
    let mut packed = scanned[0].text.clone();
    for pair in scanned.windows(2) {
        let current = &pair[0];
        let following = &pair[1];
        let left = packed.trim_end_matches([' ', '\t']);
        let right = following.text.trim_start_matches([' ', '\t']);
        if !safe_continuation_boundary(current, following, left, right) {
            packed.clone_from(&following.text);
            continue;
        }
        let separator = join_separator(left, right);
        let candidate = format!("{left}{separator}{right}");
        if visual_width(&candidate) > max_columns {
            packed.clone_from(&following.text);
            continue;
        }
        let current_content_end = current.start + current.text.trim_end_matches([' ', '\t']).len();
        edits.push(Edit::new(
            current_content_end,
            following.start + following.indent_end,
            separator,
            "COMPACT_CONTINUATION",
            format!("joined a continuation line without exceeding {max_columns} display columns"),
            Some(following.number),
        )?);
        packed = candidate;
    }
    Ok(edits)
}

fn continuation_lines(context: &ParsedContext) -> Vec<ContinuationLine> {
    let source = context.source();
    let lines = context.lines();
    let preprocessors = preprocessor_line_set(source, &lines);
    let mut splice_lines = BTreeSet::new();
    for (number, _, text) in lines.iter() {
        if has_sensitive_line_end(text) {
            splice_lines.insert(number);
            splice_lines.insert(number.saturating_add(1));
        }
    }
    let mut delimiter_depth = 0_u32;
    let mut result = Vec::new();
    for (number, line, text) in lines.iter() {
        if !preprocessors.contains(&number) {
            for (relative, byte) in text.bytes().enumerate() {
                if context.lexical().is_protected(line.start + relative) {
                    continue;
                }
                match byte {
                    b'(' | b'[' => delimiter_depth = delimiter_depth.saturating_add(1),
                    b')' | b']' => delimiter_depth = delimiter_depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        result.push(ContinuationLine {
            number,
            start: line.start,
            text: text.to_owned(),
            indent_end: leading_whitespace(text),
            has_comment: context.lexical().line_has_comment(number),
            preprocessor: preprocessors.contains(&number),
            splice_barrier: splice_lines.contains(&number),
            delimiter_depth_after: delimiter_depth,
        });
    }
    result
}

fn safe_continuation_boundary(
    current: &ContinuationLine,
    following: &ContinuationLine,
    left: &str,
    right: &str,
) -> bool {
    if left.is_empty()
        || right.is_empty()
        || current.has_comment
        || following.has_comment
        || current.preprocessor
        || following.preprocessor
        || current.splice_barrier
        || following.splice_barrier
    {
        return false;
    }
    if current.delimiter_depth_after > 0 || ends_with_continuing_operator(left) {
        return true;
    }
    let Some(operator) = leading_operator(right) else {
        return false;
    };
    // The `)` that closes a control header ends the header, not an operand, so
    // what follows is the body rather than more of the same expression. Reading
    // it as an operand joins `if (a > 0)` to a body that starts with `*`, which
    // the brace rule then puts back on its own line — the two rules undo each
    // other for as long as the run is willing to keep trying.
    ends_like_operand(left)
        && !is_control_header(left.trim_start())
        && !(matches!(operator, "*" | "&") && looks_like_declaration_prefix(left))
}

fn join_separator(left: &str, right: &str) -> &'static str {
    if left.ends_with(['(', '[']) || right.starts_with([')', ']', ',', ';']) {
        ""
    } else {
        " "
    }
}

const OPERATORS: &[&str] = &[
    "<<=", ">>=", "&&", "||", "==", "!=", "<=", ">=", "<<", ">>", "->", "+=", "-=", "*=", "/=",
    "%=", "&=", "|=", "^=", "++", "--", "+", "-", "*", "/", "%", "<", ">", "=", "!", "&", "|", "^",
    "~", "?", ":",
];

fn leading_operator(text: &str) -> Option<&'static str> {
    OPERATORS
        .iter()
        .copied()
        .find(|operator| text.starts_with(operator))
}

fn trailing_operator(text: &str) -> Option<&'static str> {
    OPERATORS
        .iter()
        .copied()
        .find(|operator| text.ends_with(operator))
}

fn ends_with_continuing_operator(text: &str) -> bool {
    let stripped = text.trim_end();
    !stripped.ends_with([','])
        && !stripped.ends_with("++")
        && !stripped.ends_with("--")
        && trailing_operator(stripped).is_some()
}

fn ends_like_operand(text: &str) -> bool {
    let stripped = text.trim_end();
    stripped.ends_with([')', ']'])
        || stripped.ends_with("++")
        || stripped.ends_with("--")
        || stripped.chars().next_back().is_some_and(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '"' | '\'')
        })
}

fn looks_like_declaration_prefix(text: &str) -> bool {
    let words: Vec<_> = text
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty())
        .collect();
    !words.is_empty()
        && words.iter().all(|word| {
            is_declaration_word(word)
                || word.starts_with("t_")
                || word.ends_with("_t")
                || (words.len() == 1 && is_identifier(word))
        })
}

#[derive(Clone, Copy, Debug)]
struct FunctionSignature {
    line: u32,
    declarator_start: usize,
    prefix_end: usize,
    definition: bool,
}

fn fix_function_layout(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
    options: &CActionOptions,
) -> Result<Vec<Edit>, CActionError> {
    let signatures = function_signatures(context);
    let target_definitions: BTreeSet<u32> = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code.as_str(),
                "SPACE_BEFORE_FUNC" | "TOO_MANY_TABS_FUNC" | "MISSING_TAB_FUNC"
            )
        })
        .map(|diagnostic| diagnostic.line)
        .collect();
    let align_prototypes = options.format_proven_declarations
        || diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MISALIGNED_FUNC_DECL");
    let lines = context.lines();
    let mut edits = Vec::new();

    for signature in signatures
        .iter()
        // One tab between the return type and the declarator is proven from
        // the signature itself: the gap is whitespace and nothing else. It was
        // gated on an official report while the identical prototype rule was
        // not, which is why a run without a checker left definitions alone.
        .filter(|signature| {
            signature.definition
                && (options.format_proven_declarations
                    || target_definitions.contains(&signature.line))
        })
    {
        let gap = context
            .source()
            .get(signature.prefix_end..signature.declarator_start)
            .unwrap_or("");
        if gap.bytes().all(|byte| matches!(byte, b' ' | b'\t')) && gap != "\t" {
            edits.push(Edit::new(
                signature.prefix_end,
                signature.declarator_start,
                "\t",
                "FUNCTION_SPACING",
                "used one tab between the return type and function declarator",
                Some(signature.line),
            )?);
        }
    }

    let prototypes: Vec<_> = signatures
        .iter()
        .filter(|signature| !signature.definition)
        .collect();
    if align_prototypes && !prototypes.is_empty() {
        let mut target_column = 9_u32;
        for signature in &prototypes {
            let Some(line) = lines.get(signature.line) else {
                continue;
            };
            let prefix_column = lines.visual_column(line, signature.prefix_end);
            target_column = target_column.max(next_tab_stop(prefix_column));
        }
        for signature in prototypes {
            let Some(line) = lines.get(signature.line) else {
                continue;
            };
            let prefix_column = lines.visual_column(line, signature.prefix_end);
            let Some(tabs) = tabs_to_column(prefix_column, target_column) else {
                continue;
            };
            let gap = context
                .source()
                .get(signature.prefix_end..signature.declarator_start)
                .unwrap_or("");
            if gap.bytes().all(|byte| matches!(byte, b' ' | b'\t')) && gap != tabs {
                let candidate_width = visual_width(lines.text(line))
                    .saturating_sub(visual_width(gap))
                    .saturating_add(visual_width_from(&tabs, prefix_column));
                if candidate_width <= options.max_columns {
                    edits.push(Edit::new(
                        signature.prefix_end,
                        signature.declarator_start,
                        tabs,
                        "MISALIGNED_FUNC_DECL",
                        "aligned a simple function prototype at the shared tab stop",
                        Some(signature.line),
                    )?);
                }
            }
        }
    }
    Ok(edits)
}

fn function_signatures(context: &ParsedContext) -> Vec<FunctionSignature> {
    let tokens = context.tokens();
    let lines = context.lines();
    let mut result = Vec::new();
    for fact in &context.facts().functions {
        let name_start = fact.name_range.start().get() as usize;
        let Some(name_index) = tokens.iter().position(|token| token.start == name_start) else {
            continue;
        };
        let line_number = lines.line_number_at(tokens[name_index].start);
        let mut first_on_line = name_index;
        while first_on_line > 0
            && lines.line_number_at(tokens[first_on_line - 1].start) == line_number
        {
            first_on_line -= 1;
        }
        if first_on_line == name_index
            || tokens[first_on_line..name_index].iter().any(|prefix| {
                matches!(
                    prefix.text.as_str(),
                    "=" | "," | "(" | ")" | "[" | "]" | "{" | "}" | ";"
                )
            })
            || matches!(
                tokens[first_on_line].text.as_str(),
                "return" | "if" | "while" | "for" | "switch"
            )
        {
            continue;
        }
        let mut declarator_index = name_index;
        while declarator_index > first_on_line && tokens[declarator_index - 1].text == "*" {
            declarator_index -= 1;
        }
        if declarator_index == first_on_line {
            continue;
        }
        let prefix_end = tokens[declarator_index - 1].end;
        let declarator_start = tokens[declarator_index].start;
        if context
            .source()
            .get(prefix_end..declarator_start)
            .is_none_or(|gap| !gap.bytes().all(|byte| matches!(byte, b' ' | b'\t')))
        {
            continue;
        }
        result.push(FunctionSignature {
            line: line_number,
            declarator_start,
            prefix_end,
            definition: fact.kind == CFunctionKind::Definition,
        });
    }
    result
}

fn next_tab_stop(column: u32) -> u32 {
    column.saturating_add(4 - ((column.saturating_sub(1)) % 4))
}

fn tabs_to_column(mut column: u32, target: u32) -> Option<String> {
    if column >= target {
        return None;
    }
    let mut tabs = String::new();
    while column < target {
        tabs.push('\t');
        column = next_tab_stop(column);
    }
    (column == target).then_some(tabs)
}

#[derive(Clone, Copy, Debug, Default)]
struct IndentInfo {
    expected: u32,
    brace_depth: u32,
    delimiter_depth: u32,
    continuation_extra: u32,
    continuation: bool,
}

fn indentation_model(context: &ParsedContext) -> BTreeMap<u32, IndentInfo> {
    let lines = context.lines();
    let preprocessors = preprocessor_line_set(context.source(), &lines);
    let mut model = BTreeMap::new();
    let mut brace_depth = 0_u32;
    let mut delimiter_depth = 0_u32;
    let mut continued = false;
    // Braceless control headers nest, and each one indents whatever follows it
    // by another level. Counting them separately from an expression that spills
    // onto the next line is what makes both come out right: `if` inside `if`
    // inside `if` is three levels deep, while `a +` over three lines is one.
    let mut open_headers = 0_u32;
    for (line_number, line, text) in lines.iter() {
        let stripped = text.trim_start_matches([' ', '\t']);
        let closes = stripped.starts_with('}');
        let line_brace_depth = brace_depth.saturating_sub(u32::from(closes));
        let continuation_extra = if delimiter_depth == 0 && !stripped.starts_with('{') {
            open_headers.saturating_add(u32::from(continued))
        } else {
            0
        };
        let continuation = delimiter_depth > 0 || continuation_extra > 0;
        model.insert(
            line_number,
            IndentInfo {
                expected: line_brace_depth
                    .saturating_add(delimiter_depth)
                    .saturating_add(continuation_extra),
                brace_depth: line_brace_depth,
                delimiter_depth,
                continuation_extra,
                continuation,
            },
        );
        if preprocessors.contains(&line_number) {
            continued = false;
            continue;
        }
        for (relative, byte) in text.bytes().enumerate() {
            if context.lexical().is_protected(line.start + relative) {
                continue;
            }
            match byte {
                b'{' => brace_depth = brace_depth.saturating_add(1),
                b'}' => brace_depth = brace_depth.saturating_sub(1),
                b'(' | b'[' => delimiter_depth = delimiter_depth.saturating_add(1),
                b')' | b']' => delimiter_depth = delimiter_depth.saturating_sub(1),
                _ => {}
            }
        }
        let code = stripped.trim_end();
        let statement_ends =
            code.is_empty() || matches!(code.as_bytes().last(), Some(b';' | b'{' | b'}' | b':'));
        if statement_ends {
            open_headers = 0;
        } else if is_control_header(code) {
            open_headers = open_headers.saturating_add(1);
        }
        continued = !statement_ends && !is_control_header(code);
    }
    model
}

/// Whether this line opens a control structure whose body has no braces.
///
/// The distinction matters because such a header indents the statement after
/// it, and unlike an unfinished expression it can stack: the body of an `if`
/// may itself be another `if`.
fn is_control_header(code: &str) -> bool {
    const HEADED: [&str; 4] = ["if", "while", "for", "switch"];
    if code == "do" || code == "else" {
        return true;
    }
    let head = code.strip_prefix("else ").unwrap_or(code);
    code.ends_with(')')
        && HEADED.iter().any(|keyword| {
            head.strip_prefix(keyword)
                .is_some_and(|rest| rest.starts_with([' ', '(']))
        })
}

#[allow(clippy::too_many_lines)]
fn fix_indentation(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    let model = indentation_model(context);
    let preprocessors = preprocessor_line_set(context.source(), &lines);
    let mut by_line: BTreeMap<u32, BTreeSet<&str>> = BTreeMap::new();
    for diagnostic in diagnostics {
        by_line
            .entry(diagnostic.line)
            .or_default()
            .insert(diagnostic.code.as_str());
    }
    let lexical = context.lexical();
    for (line_number, line, text) in lines.iter() {
        let leading_end = leading_whitespace(text);
        let leading = &text[..leading_end];
        if leading.contains(' ') && leading.contains('\t') {
            by_line
                .entry(line_number)
                .or_default()
                .insert("MIXED_SPACE_TAB");
        // Indentation is derived from the syntax, not read off an official
        // report, so a run without a checker reaches the same answer. The
        // guard is that the line must actually start code: leading spaces
        // inside a string or a block comment are content.
        } else if leading.contains(' ')
            && leading_end < text.len()
            && !lexical.is_protected(line.start + leading_end)
        {
            by_line
                .entry(line_number)
                .or_default()
                .insert("SPACE_REPLACE_TAB");
        }
    }
    let mut edits = Vec::new();
    for (line_number, codes) in by_line {
        if preprocessors.contains(&line_number) {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let text = lines.text(line);
        let leading_end = leading_whitespace(text);
        let raw_leading = &text[..leading_end];
        let expected_count = model.get(&line_number).map_or(0, |info| info.expected);
        let expected = "\t".repeat(expected_count as usize);

        if codes.contains("MIXED_SPACE_TAB") {
            if raw_leading.contains(' ') && raw_leading.contains('\t') && raw_leading != expected {
                edits.push(Edit::new(
                    line.start,
                    line.start + leading_end,
                    expected.clone(),
                    "MIXED_SPACE_TAB",
                    "replaced mixed leading whitespace with syntax-derived indentation tabs",
                    Some(line_number),
                )?);
                continue;
            }
            if let Some(diagnostic) = diagnostics
                .iter()
                .find(|item| item.line == line_number && item.code == "MIXED_SPACE_TAB")
            {
                let position = lines.byte_for_visual_column(line, diagnostic.visual_column);
                let relative = position.saturating_sub(line.start);
                let (start, end) = whitespace_run_near(text, relative);
                let run = &text[start..end];
                if run.contains(' ') && run.contains('\t') {
                    let width =
                        visual_width_from(run, lines.visual_column(line, line.start + start));
                    edits.push(Edit::new(
                        line.start + start,
                        line.start + end,
                        "\t".repeat(width.div_ceil(4).max(1) as usize),
                        "MIXED_SPACE_TAB",
                        "replaced mixed internal whitespace with tabs",
                        Some(line_number),
                    )?);
                    continue;
                }
            }
        }
        if codes.contains("SPACE_REPLACE_TAB") && leading_end > 0 && raw_leading.contains(' ') {
            edits.push(Edit::new(
                line.start,
                line.start + leading_end,
                expected.clone(),
                "SPACE_REPLACE_TAB",
                "replaced indentation spaces with syntax-derived tabs",
                Some(line_number),
            )?);
            continue;
        }
        if codes.contains("TOO_FEW_TAB") && raw_leading != expected {
            edits.push(Edit::new(
                line.start,
                line.start + leading_end,
                expected.clone(),
                "TOO_FEW_TAB",
                "set indentation from surrounding syntax depth",
                Some(line_number),
            )?);
            continue;
        }
        if codes.contains("TOO_MANY_TAB") && !raw_leading.is_empty() {
            let replacement = if visual_width(&expected) < visual_width(raw_leading) {
                expected.clone()
            } else if raw_leading.bytes().all(|byte| byte == b'\t') {
                raw_leading[..raw_leading.len() - 1].to_owned()
            } else {
                continue;
            };
            edits.push(Edit::new(
                line.start,
                line.start + leading_end,
                replacement,
                "TOO_MANY_TAB",
                "removed extra leading indentation using syntax depth",
                Some(line_number),
            )?);
            continue;
        }
        if codes.contains("TAB_REPLACE_SPACE") {
            let diagnostic = diagnostics
                .iter()
                .find(|item| item.line == line_number && item.code == "TAB_REPLACE_SPACE");
            if let Some(diagnostic) = diagnostic {
                let offset = lines.byte_for_visual_column(line, diagnostic.visual_column);
                let mut relative = offset.saturating_sub(line.start).min(text.len());
                if text.as_bytes().get(relative) != Some(&b'\t') {
                    relative = text[relative.saturating_sub(1)..]
                        .find('\t')
                        .map_or(relative, |found| relative.saturating_sub(1) + found);
                }
                if text.as_bytes().get(relative) == Some(&b'\t') {
                    edits.push(Edit::new(
                        line.start + relative,
                        line.start + relative + 1,
                        " ",
                        "TAB_REPLACE_SPACE",
                        "replaced an alignment tab with a natural space",
                        Some(line_number),
                    )?);
                }
            }
        }
        for code in ["MISSING_TAB_VAR", "MISSING_TAB_TYPDEF", "NO_TAB_BF_TYPEDEF"] {
            if let Some(diagnostic) = diagnostics
                .iter()
                .find(|item| item.line == line_number && item.code == code)
            {
                let offset = lines.byte_for_visual_column(line, diagnostic.visual_column);
                let relative = offset.saturating_sub(line.start);
                let (start, end) = whitespace_before(text, relative);
                if start != end {
                    edits.push(Edit::new(
                        line.start + start,
                        line.start + end,
                        "\t",
                        code,
                        "inserted the required declaration tab",
                        Some(line_number),
                    )?);
                }
            }
        }
    }
    Ok(edits)
}

fn whitespace_run_near(text: &str, index: usize) -> (usize, usize) {
    let bytes = text.as_bytes();
    let mut start = index.min(bytes.len());
    if start == bytes.len() || !matches!(bytes[start], b' ' | b'\t') {
        start = start.saturating_sub(1);
    }
    while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    let mut end = index.min(bytes.len());
    while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
        end += 1;
    }
    (start, end)
}

#[allow(clippy::too_many_lines)]
fn fix_token_spacing(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    let blocked = multiline_preprocessor_lines(context.source(), &lines);
    let mut edits = Vec::new();
    for diagnostic in diagnostics {
        if blocked.contains(&diagnostic.line) {
            continue;
        }
        let Some(line) = lines.get(diagnostic.line) else {
            continue;
        };
        let text = lines.text(line);
        let byte = lines.byte_for_visual_column(line, diagnostic.visual_column);
        let relative = byte.saturating_sub(line.start).min(text.len());
        let code = diagnostic.code.as_str();
        if matches!(
            code,
            "SPC_BFR_OPERATOR"
                | "SPC_AFTER_OPERATOR"
                | "NO_SPC_BFR_OPR"
                | "NO_SPC_AFR_OPR"
                | "SPC_BFR_POINTER"
                | "SPC_AFTER_POINTER"
        ) {
            let Some((start, end)) = operator_span(text, line.start, relative, context.lexical())
            else {
                continue;
            };
            let operator = &text[start..end];
            if matches!(operator, "+" | "-") && inside_numeric_exponent(text, start) {
                continue;
            }
            match code {
                "SPC_BFR_OPERATOR" | "SPC_BFR_POINTER" => {
                    let (space_start, _) = whitespace_before(text, start);
                    if space_start == start {
                        edits.push(Edit::new(
                            line.start + start,
                            line.start + start,
                            " ",
                            code,
                            "inserted required space before an operator",
                            Some(diagnostic.line),
                        )?);
                    }
                }
                "SPC_AFTER_OPERATOR" => {
                    let (_, space_end) = whitespace_after(text, end);
                    if space_end == end {
                        edits.push(Edit::new(
                            line.start + end,
                            line.start + end,
                            " ",
                            code,
                            "inserted required space after an operator",
                            Some(diagnostic.line),
                        )?);
                    }
                }
                "NO_SPC_BFR_OPR" => {
                    let (space_start, space_end) = whitespace_before(text, start);
                    let void_return = operator == ";"
                        && text[..start]
                            .trim_end_matches([' ', '\t'])
                            .ends_with("return");
                    if space_start != space_end && !void_return {
                        edits.push(Edit::new(
                            line.start + space_start,
                            line.start + space_end,
                            "",
                            code,
                            "removed forbidden space before an operator",
                            Some(diagnostic.line),
                        )?);
                    }
                }
                "NO_SPC_AFR_OPR" | "SPC_AFTER_POINTER" => {
                    let (space_start, space_end) = whitespace_after(text, end);
                    if space_start != space_end {
                        edits.push(Edit::new(
                            line.start + space_start,
                            line.start + space_end,
                            "",
                            code,
                            "removed forbidden space after an operator",
                            Some(diagnostic.line),
                        )?);
                    }
                }
                _ => {}
            }
            continue;
        }
        if matches!(
            code,
            "SPC_BFR_PAR" | "SPC_AFTER_PAR" | "NO_SPC_BFR_PAR" | "NO_SPC_AFR_PAR"
        ) {
            let Some(parenthesis) = parenthesis_near(text, line.start, relative, context.lexical())
            else {
                continue;
            };
            match code {
                "SPC_BFR_PAR" => {
                    let (start, end) = whitespace_before(text, parenthesis);
                    if start == end {
                        edits.push(Edit::new(
                            line.start + parenthesis,
                            line.start + parenthesis,
                            " ",
                            code,
                            "inserted required space before a parenthesis",
                            Some(diagnostic.line),
                        )?);
                    }
                }
                "SPC_AFTER_PAR" => {
                    let (start, end) = whitespace_after(text, parenthesis + 1);
                    if start == end {
                        edits.push(Edit::new(
                            line.start + parenthesis + 1,
                            line.start + parenthesis + 1,
                            " ",
                            code,
                            "inserted required space after a parenthesis",
                            Some(diagnostic.line),
                        )?);
                    }
                }
                "NO_SPC_BFR_PAR" => {
                    let (start, end) = whitespace_before(text, parenthesis);
                    if start != end {
                        edits.push(Edit::new(
                            line.start + start,
                            line.start + end,
                            "",
                            code,
                            "removed forbidden space before a parenthesis",
                            Some(diagnostic.line),
                        )?);
                    }
                }
                "NO_SPC_AFR_PAR" => {
                    let (start, end) = whitespace_after(text, parenthesis + 1);
                    if start != end {
                        edits.push(Edit::new(
                            line.start + start,
                            line.start + end,
                            "",
                            code,
                            "removed forbidden space after a parenthesis",
                            Some(diagnostic.line),
                        )?);
                    }
                }
                _ => {}
            }
            continue;
        }
        match code {
            "CONSECUTIVE_SPC" | "CONSECUTIVE_WS" => {
                let (start, end) = whitespace_run_near(text, relative);
                if end.saturating_sub(start) > 1 {
                    edits.push(Edit::new(
                        line.start + start,
                        line.start + end,
                        " ",
                        code,
                        "collapsed consecutive whitespace",
                        Some(diagnostic.line),
                    )?);
                }
            }
            "TAB_INSTEAD_SPC" => {
                let probe = find_near_byte(text, relative, b'\t');
                if let Some(probe) = probe {
                    edits.push(Edit::new(
                        line.start + probe,
                        line.start + probe + 1,
                        " ",
                        code,
                        "replaced a tab with a natural space",
                        Some(diagnostic.line),
                    )?);
                }
            }
            "SPACE_AFTER_KW" => {
                let mut start = relative;
                while start > 0
                    && (text.as_bytes()[start - 1].is_ascii_alphanumeric()
                        || text.as_bytes()[start - 1] == b'_')
                {
                    start -= 1;
                }
                let mut end = start;
                while text
                    .as_bytes()
                    .get(end)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    end += 1;
                }
                if end > start && !matches!(text.as_bytes().get(end), Some(b' ' | b'\t')) {
                    edits.push(Edit::new(
                        line.start + end,
                        line.start + end,
                        " ",
                        code,
                        "inserted required space after a keyword",
                        Some(diagnostic.line),
                    )?);
                }
            }
            "SPC_LINE_START" => {
                let end = leading_whitespace(text);
                if end > 0 {
                    edits.push(Edit::new(
                        line.start,
                        line.start + end,
                        "",
                        code,
                        "removed unexpected leading whitespace",
                        Some(diagnostic.line),
                    )?);
                }
            }
            _ => {}
        }
    }
    Ok(edits)
}

fn multiline_preprocessor_lines(_source: &str, lines: &SourceLines<'_>) -> BTreeSet<u32> {
    let mut result = BTreeSet::new();
    let mut active = false;
    for (number, _, text) in lines.iter() {
        let starts = text.trim_start_matches([' ', '\t']).starts_with('#');
        let continues = has_sensitive_line_end(text);
        if active {
            result.insert(number);
            active = continues;
        } else if starts && continues {
            result.insert(number);
            active = true;
        }
    }
    result
}

fn operator_span(
    text: &str,
    base: usize,
    index: usize,
    lexical: &LexicalMap,
) -> Option<(usize, usize)> {
    const SPACING_OPERATORS: &[&str] = &[
        ">>=", "<<=", "...", "++", "--", "->", "&&", "||", "==", "!=", "<=", ">=", "+=", "-=",
        "*=", "/=", "%=", "&=", "|=", "^=", "<<", ">>", "+", "-", "*", "/", "%", "<", ">", "=",
        "!", "&", "|", "^", "~", "?", ":", ",", ";", ".",
    ];
    for probe in [index, index.saturating_sub(1), index.saturating_sub(2)] {
        for operator in SPACING_OPERATORS {
            let end = probe.saturating_add(operator.len());
            if text.get(probe..end) == Some(*operator)
                && !(probe..end).any(|offset| lexical.is_protected(base + offset))
            {
                return Some((probe, end));
            }
        }
    }
    None
}

fn parenthesis_near(text: &str, base: usize, index: usize, lexical: &LexicalMap) -> Option<usize> {
    let lower = index.saturating_sub(2);
    let upper = (index + 3).min(text.len());
    (lower..upper).find(|position| {
        matches!(
            text.as_bytes()[*position],
            b'(' | b')' | b'[' | b']' | b'{' | b'}'
        ) && !lexical.is_protected(base + *position)
    })
}

fn find_near_byte(text: &str, index: usize, byte: u8) -> Option<usize> {
    if text.as_bytes().get(index) == Some(&byte) {
        return Some(index);
    }
    let start = index.saturating_sub(1);
    text.as_bytes()[start..]
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|position| start + position)
}

fn inside_numeric_exponent(text: &str, operator: usize) -> bool {
    if operator < 2 || operator + 1 >= text.len() {
        return false;
    }
    let bytes = text.as_bytes();
    let marker = bytes[operator - 1];
    if !matches!(marker, b'e' | b'E' | b'p' | b'P') || !bytes[operator + 1].is_ascii_digit() {
        return false;
    }
    let mut start = operator - 1;
    while start > 0
        && (bytes[start - 1].is_ascii_hexdigit() || matches!(bytes[start - 1], b'x' | b'X' | b'.'))
    {
        start -= 1;
    }
    if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        return false;
    }
    let literal = &text[start..operator - 1];
    if matches!(marker, b'e' | b'E') {
        decimal_mantissa_regex().is_match(literal)
    } else {
        hex_mantissa_regex().is_match(literal)
    }
}

fn format_initial_declarations(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    let mut edits = Vec::new();
    for declaration in context
        .facts()
        .local_declarations
        .iter()
        .filter(|declaration| declaration.initial)
    {
        let start = declaration.range.start().get() as usize;
        let end = declaration.range.end().get() as usize;
        let line_number = lines.line_number_at(start);
        if lines.line_number_at(end.saturating_sub(1)) != line_number {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let leading_end = leading_whitespace(lines.text(line));
        if line.start + leading_end != start {
            continue;
        }
        let leading = &context.source()[line.start..start];
        if leading != "\t" {
            edits.push(Edit::new(
                line.start,
                start,
                "\t",
                "INITIAL_DECLARATION_INDENT",
                "indented an initial local declaration with one tab",
                Some(line_number),
            )?);
        }
    }
    for block in &context.facts().initial_declaration_blocks {
        let Some(last) = block.declarations.last() else {
            continue;
        };
        let Some(following) = block.following_item else {
            continue;
        };
        let declaration_line = lines.line_number_at(last.end().get() as usize - 1);
        let following_line = lines.line_number_at(following.start().get() as usize);
        if following_line != declaration_line.saturating_add(1) {
            continue;
        }
        let Some(line) = lines.get(following_line) else {
            continue;
        };
        edits.push(Edit::new(
            line.start,
            line.start,
            "\n",
            "INITIAL_DECLARATION_BLANK_LINE",
            "inserted one blank line after the initial declaration block",
            Some(following_line),
        )?);
    }
    Ok(edits)
}

fn decimal_mantissa_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^(?:\d+(?:\.\d*)?|\.\d+)$").expect("constant decimal regex is valid")
    })
}

fn hex_mantissa_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^0[xX](?:[0-9A-Fa-f]+(?:\.[0-9A-Fa-f]*)?|\.[0-9A-Fa-f]+)$")
            .expect("constant hexadecimal regex is valid")
    })
}

#[derive(Clone, Debug)]
struct Declaration {
    line: u32,
    scope: Vec<u32>,
    text: String,
    offset: usize,
    gap_start: usize,
    gap_end: usize,
    prefix_column: u32,
    declarator_column: u32,
}

fn align_declarations(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
    options: &CActionOptions,
) -> Result<Vec<Edit>, CActionError> {
    let targets: BTreeSet<u32> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "MISALIGNED_VAR_DECL")
        .map(|diagnostic| diagnostic.line)
        .collect();
    if targets.is_empty() && !options.format_proven_declarations {
        return Ok(Vec::new());
    }
    let groups = declaration_groups(context);
    let mut edits = Vec::new();
    for group in groups {
        if group.is_empty()
            || (!targets.is_empty()
                && !options.format_proven_declarations
                && group.iter().all(|item| !targets.contains(&item.line)))
        {
            continue;
        }
        let minimum = group
            .iter()
            .map(|item| next_tab_stop(item.prefix_column))
            .max()
            .unwrap_or(1);
        let anchor = group[0].declarator_column;
        let target = if anchor >= minimum
            && group
                .iter()
                .all(|item| tabs_to_column(item.prefix_column, anchor).is_some())
        {
            anchor
        } else {
            minimum
        };
        let mut group_edits = Vec::new();
        let mut safe = true;
        for declaration in &group {
            let Some(tabs) = tabs_to_column(declaration.prefix_column, target) else {
                safe = false;
                break;
            };
            let rebuilt = format!(
                "{}{}{}",
                &declaration.text[..declaration.gap_start],
                tabs,
                &declaration.text[declaration.gap_end..]
            );
            if visual_width(&rebuilt) > options.max_columns {
                safe = false;
                break;
            }
            group_edits.push(Edit::new(
                declaration.offset + declaration.gap_start,
                declaration.offset + declaration.gap_end,
                tabs,
                "MISALIGNED_VAR_DECL",
                "aligned a simple declaration group with tabs",
                Some(declaration.line),
            )?);
        }
        if safe {
            edits.extend(group_edits);
        }
    }
    Ok(edits)
}

fn declaration_groups(context: &ParsedContext) -> Vec<Vec<Declaration>> {
    let lines = context.lines();
    let mut scope = vec![0_u32];
    let mut next_scope = 1_u32;
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for (line_number, line, text) in lines.iter() {
        let has_protected =
            (line.start..line.content_end).any(|offset| context.lexical().is_protected(offset));
        let declaration =
            parse_declaration(text, line_number, line.start, scope.clone(), has_protected);
        let continues_group = declaration.as_ref().is_some_and(|item| {
            current.last().is_some_and(|previous: &Declaration| {
                item.scope == previous.scope && item.line == previous.line.saturating_add(1)
            })
        });
        if let Some(declaration) = declaration {
            if !continues_group && !current.is_empty() {
                groups.push(std::mem::take(&mut current));
            }
            current.push(declaration);
        } else if !current.is_empty() {
            groups.push(std::mem::take(&mut current));
        }

        if !text.trim_start_matches([' ', '\t']).starts_with('#') {
            for (relative, byte) in text.bytes().enumerate() {
                if context.lexical().is_protected(line.start + relative) {
                    continue;
                }
                if byte == b'}' {
                    if scope.len() > 1 {
                        scope.pop();
                    }
                } else if byte == b'{' {
                    scope.push(next_scope);
                    next_scope = next_scope.saturating_add(1);
                }
            }
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn parse_declaration(
    text: &str,
    line: u32,
    offset: usize,
    scope: Vec<u32>,
    has_protected: bool,
) -> Option<Declaration> {
    if has_protected
        || text
            .bytes()
            .any(|byte| matches!(byte, b',' | b'(' | b')' | b':' | b'\\' | b'{' | b'}'))
        || text.bytes().filter(|byte| *byte == b';').count() != 1
        || !text.trim_end().ends_with(';')
    {
        return None;
    }
    let captures = simple_declaration_regex().captures(text)?;
    let gap = captures.name("gap")?;
    let declarator = captures.name("declarator")?;
    Some(Declaration {
        line,
        scope,
        text: text.to_owned(),
        offset,
        gap_start: gap.start(),
        gap_end: gap.end(),
        prefix_column: visual_width(&text[..gap.start()]) + 1,
        declarator_column: visual_width(&text[..declarator.start()]) + 1,
    })
}

fn simple_declaration_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?x)
            ^
            (?P<indent>\t*)
            (?P<type>
                (?:
                    (?:(?:static|extern|register|const|volatile|restrict|signed|unsigned|short|long)\x20+)*
                    (?:
                        (?:struct|union|enum)\x20+[A-Za-z_][A-Za-z0-9_]*
                        |void|char|int|float|double|_Bool|short|long|signed|unsigned
                        |va_list|size_t|ssize_t|ptrdiff_t|bool|FILE
                        |t_[A-Za-z0-9_]+|[A-Za-z_][A-Za-z0-9_]*_t|[A-Z][A-Za-z0-9_]*
                    )
                    (?:\x20+(?:const|volatile|restrict|signed|unsigned|short|long|int))*
                )
            )
            (?P<gap>[\x20\t]+)
            (?P<declarator>\*+[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*)
            (?P<arrays>(?:\[[0-9A-Z_+\-*/\x20\t]*\])*)
            (?P<initializer>[\x20\t]*=[\x20\t]*[A-Za-z0-9_+\-*/%&|^~!.<>\x20\t]+)?
            [\x20\t]*;
            $
            ",
        )
        .expect("constant simple declaration regex is valid")
    })
}

fn is_declaration_word(word: &str) -> bool {
    matches!(
        word,
        "_Atomic"
            | "_Bool"
            | "auto"
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
    )
}

fn replace_pointer_zero_returns(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    let mut edits = Vec::new();
    for returned in context
        .facts()
        .returns
        .iter()
        .filter(|returned| returned.function_returns_pointer)
    {
        let Some(expression) = returned.expression_range else {
            continue;
        };
        let expression_start = expression.start().get() as usize;
        if !context.null_is_proven_available_at(expression_start) {
            continue;
        }
        let tokens = context
            .tokens()
            .iter()
            .filter(|token| {
                token.start >= expression_start && token.end <= expression.end().get() as usize
            })
            .collect::<Vec<_>>();
        let zero = match tokens.as_slice() {
            [zero] if zero.text == "0" => Some(*zero),
            [open, zero, close] if open.text == "(" && zero.text == "0" && close.text == ")" => {
                Some(*zero)
            }
            _ => None,
        };
        let Some(zero) = zero else {
            continue;
        };
        edits.push(Edit::new(
            zero.start,
            zero.end,
            "NULL",
            "POINTER_NULL_RETURN",
            "used NULL for a proven pointer return",
            Some(context.lines().line_number_at(zero.start)),
        )?);
    }
    Ok(edits)
}

fn compact_null_checks(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
    context
        .facts()
        .null_checks
        .iter()
        .map(|check| {
            let start = check.range.start().get() as usize;
            let end = check.range.end().get() as usize;
            Edit::new(
                start,
                end,
                if check.equals {
                    format!("!{}", check.operand)
                } else {
                    format!("!!{}", check.operand)
                },
                "COMPACT_NULL_CHECK",
                "compacted an explicit NULL comparison after unsafe opt-in",
                Some(context.lines().line_number_at(start)),
            )
            .map_err(CActionError::from)
        })
        .collect()
}

fn parenthesize_returns(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
) -> Result<Vec<Edit>, CActionError> {
    let targets: BTreeSet<u32> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "RETURN_PARENTHESIS")
        .map(|diagnostic| diagnostic.line)
        .collect();
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let tokens = context.tokens();
    let lines = context.lines();
    let blocked = preprocessor_line_set(context.source(), &lines);
    let mut edits = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "return" {
            continue;
        }
        let line_number = lines.line_number_at(token.start);
        if !targets.contains(&line_number) || blocked.contains(&line_number) {
            continue;
        }
        let Some(semicolon) = statement_semicolon(tokens, index + 1) else {
            continue;
        };
        if semicolon == index + 1 {
            continue;
        }
        let expression_start = index + 1;
        let expression_end = semicolon - 1;
        if tokens[expression_start].text == "("
            && matching_forward(tokens, expression_start, "(", ")") == Some(expression_end)
        {
            continue;
        }
        if (token.end..tokens[expression_start].start)
            .any(|offset| context.lexical().is_protected(offset))
        {
            continue;
        }
        edits.push(Edit::new(
            tokens[expression_start].start,
            tokens[expression_start].start,
            "(",
            "RETURN_PARENTHESIS",
            "wrapped the complete return expression in parentheses",
            Some(line_number),
        )?);
        edits.push(Edit::new(
            tokens[semicolon].start,
            tokens[semicolon].start,
            ")",
            "RETURN_PARENTHESIS",
            "wrapped the complete return expression in parentheses",
            Some(line_number),
        )?);
    }
    Ok(edits)
}

fn statement_semicolon(tokens: &[Token], start: usize) -> Option<usize> {
    let mut parentheses = 0_u32;
    let mut brackets = 0_u32;
    let mut braces = 0_u32;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.text.as_str() {
            "(" => parentheses = parentheses.saturating_add(1),
            ")" => parentheses = parentheses.saturating_sub(1),
            "[" => brackets = brackets.saturating_add(1),
            "]" => brackets = brackets.saturating_sub(1),
            "{" => braces = braces.saturating_add(1),
            "}" if braces == 0 => return None,
            "}" => braces = braces.saturating_sub(1),
            ";" if parentheses == 0 && brackets == 0 && braces == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn add_void_to_definitions(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
) -> Result<Vec<Edit>, CActionError> {
    let targets: BTreeSet<u32> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "NO_ARGS_VOID")
        .map(|diagnostic| diagnostic.line)
        .collect();
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let lines = context.lines();
    let mut edits = Vec::new();
    for fact in context
        .facts()
        .functions
        .iter()
        .filter(|fact| fact.kind == CFunctionKind::Definition)
    {
        let line_number = lines.line_number_at(fact.name_range.start().get() as usize);
        if !targets.contains(&line_number) {
            continue;
        }
        let start = fact.parameters_range.start().get() as usize;
        let end = fact.parameters_range.end().get() as usize;
        let Some(parameters) = context.source().get(start..end) else {
            continue;
        };
        if !parameters.starts_with('(')
            || !parameters.ends_with(')')
            || !parameters[1..parameters.len() - 1].trim().is_empty()
        {
            continue;
        }
        edits.push(Edit::new(
            start + 1,
            end - 1,
            "void",
            "NO_ARGS_VOID",
            "made an empty function definition parameter list explicit",
            Some(line_number),
        )?);
    }
    Ok(edits)
}

#[derive(Clone, Copy, Debug)]
struct BreakCandidate {
    priority: u8,
    nesting: u32,
    prefix_width: u32,
    start: usize,
    end: usize,
}

fn wrap_long_lines(context: &ParsedContext, max_columns: u32) -> Result<Vec<Edit>, CActionError> {
    if max_columns == 0 {
        return Ok(Vec::new());
    }
    let lines = context.lines();
    let preprocessors = preprocessor_line_set(context.source(), &lines);
    let model = indentation_model(context);
    let mut edits = Vec::new();
    for (line_number, line, text) in lines.iter() {
        if visual_width(text) <= max_columns
            || preprocessors.contains(&line_number)
            || context.lexical().line_has_comment(line_number)
        {
            continue;
        }
        let initial_delimiter = model
            .get(&line_number)
            .map_or(0, |information| information.delimiter_depth);
        let mut candidates = Vec::new();
        scan_operator_breaks(
            context,
            line,
            text,
            initial_delimiter,
            max_columns,
            &mut candidates,
        );
        scan_comma_breaks(
            context,
            line,
            text,
            initial_delimiter,
            max_columns,
            &mut candidates,
        );
        let Some(best) = select_break(&candidates) else {
            continue;
        };
        let delimiter = delimiter_depth_at(context, line, text, best.start, initial_delimiter);
        let information = model.get(&line_number).copied().unwrap_or_default();
        let continuation_depth = delimiter.saturating_add(information.continuation_extra);
        let mut continuation_level = information
            .brace_depth
            .saturating_add(continuation_depth.max(1));
        if information.continuation {
            continuation_level = continuation_level.max(information.expected);
        }
        edits.push(Edit::new(
            line.start + best.start,
            line.start + best.end,
            format!("\n{}", "\t".repeat(continuation_level as usize)),
            "LINE_TOO_LONG",
            "wrapped a long line at a token-safe operator or comma",
            Some(line_number),
        )?);
    }
    Ok(edits)
}

fn scan_operator_breaks(
    context: &ParsedContext,
    line: PhysicalLine,
    text: &str,
    initial_delimiter: u32,
    max_columns: u32,
    candidates: &mut Vec<BreakCandidate>,
) {
    const BREAK_OPERATORS: &[&str] = &[
        "<<=", ">>=", "&&", "||", "==", "!=", "<=", ">=", "<<", ">>", "->", "+=", "-=", "*=", "/=",
        "%=", "&=", "|=", "^=", "++", "--", "+", "-", "*", "/", "%", "<", ">", "|", "^", "&", "=",
    ];
    let mut index = 0;
    while index < text.len() {
        if context.lexical().is_protected(line.start + index) {
            index += 1;
            continue;
        }
        let Some(operator) = BREAK_OPERATORS
            .iter()
            .copied()
            .find(|operator| text.as_bytes()[index..].starts_with(operator.as_bytes()))
        else {
            index += 1;
            continue;
        };
        let end = index + operator.len();
        if matches!(operator, "++" | "--")
            || (matches!(operator, "+" | "-" | "*" | "&")
                && looks_unary(context, line, text, index))
            || (matches!(operator, "+" | "-") && inside_numeric_exponent(text, index))
        {
            index = end;
            continue;
        }
        let (whitespace_start, _) = whitespace_before(text, index);
        let prefix_width = visual_width(text[..whitespace_start].trim_end());
        if (12..=max_columns).contains(&prefix_width) {
            candidates.push(BreakCandidate {
                priority: u8::from(!matches!(operator, "&&" | "||")) * 2,
                nesting: delimiter_depth_at(
                    context,
                    line,
                    text,
                    whitespace_start,
                    initial_delimiter,
                ),
                prefix_width,
                start: whitespace_start,
                end: index,
            });
        }
        index = end;
    }
}

fn scan_comma_breaks(
    context: &ParsedContext,
    line: PhysicalLine,
    text: &str,
    initial_delimiter: u32,
    max_columns: u32,
    candidates: &mut Vec<BreakCandidate>,
) {
    for (index, byte) in text.bytes().enumerate() {
        if byte != b',' || context.lexical().is_protected(line.start + index) {
            continue;
        }
        let (start, end) = whitespace_after(text, index + 1);
        let prefix_width = visual_width(&text[..=index]);
        if (12..=max_columns).contains(&prefix_width) {
            candidates.push(BreakCandidate {
                priority: 1,
                nesting: delimiter_depth_at(context, line, text, index, initial_delimiter),
                prefix_width,
                start,
                end,
            });
        }
    }
}

fn select_break(candidates: &[BreakCandidate]) -> Option<BreakCandidate> {
    let priority = candidates
        .iter()
        .map(|candidate| candidate.priority)
        .min()?;
    let nesting = candidates
        .iter()
        .filter(|candidate| candidate.priority == priority)
        .map(|candidate| candidate.nesting)
        .min()?;
    candidates
        .iter()
        .filter(|candidate| candidate.priority == priority && candidate.nesting == nesting)
        .max_by_key(|candidate| candidate.prefix_width)
        .copied()
}

fn delimiter_depth_at(
    context: &ParsedContext,
    line: PhysicalLine,
    text: &str,
    end: usize,
    initial: u32,
) -> u32 {
    let mut depth = initial;
    for (relative, byte) in text.bytes().take(end).enumerate() {
        if context.lexical().is_protected(line.start + relative) {
            continue;
        }
        match byte {
            b'(' | b'[' => depth = depth.saturating_add(1),
            b')' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

fn looks_unary(context: &ParsedContext, line: PhysicalLine, text: &str, operator: usize) -> bool {
    let mut probe = operator;
    while probe > 0 {
        probe -= 1;
        if context.lexical().is_protected(line.start + probe)
            || matches!(text.as_bytes()[probe], b' ' | b'\t')
        {
            continue;
        }
        return b"([{,=?:!~+-*/%&|^<>".contains(&text.as_bytes()[probe]);
    }
    true
}
