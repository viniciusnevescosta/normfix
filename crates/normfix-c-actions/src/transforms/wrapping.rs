use super::{
    CActionError, Edit, ParsedContext, PhysicalLine, indentation_model, inside_numeric_exponent,
    preprocessor_line_set, visual_width, whitespace_after, whitespace_before,
};

#[derive(Clone, Copy, Debug)]
struct BreakCandidate {
    priority: u8,
    nesting: u32,
    prefix_width: u32,
    start: usize,
    end: usize,
}

pub(super) fn wrap_long_lines(
    context: &ParsedContext,
    max_columns: u32,
) -> Result<Vec<Edit>, CActionError> {
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
