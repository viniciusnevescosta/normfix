use super::{
    BTreeSet, CActionError, Edit, ParsedContext, ReportedDiagnostic, TextRange,
    control_condition_close, find_brace_near, leading_whitespace, preprocessor_line_set,
    whitespace_after, whitespace_before,
};

pub(super) fn fix_blank_lines(
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

pub(super) fn fix_braces_and_controls(
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
