use super::{
    BTreeSet, CActionError, Edit, LexicalMap, ParsedContext, ReportedDiagnostic, SourceLines,
    decimal_mantissa_regex, has_sensitive_line_end, hex_mantissa_regex, leading_whitespace,
    whitespace_after, whitespace_before, whitespace_run_near,
};

#[allow(clippy::too_many_lines)]
pub(super) fn fix_token_spacing(
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

pub(super) fn multiline_preprocessor_lines(
    _source: &str,
    lines: &SourceLines<'_>,
) -> BTreeSet<u32> {
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

pub(super) fn inside_numeric_exponent(text: &str, operator: usize) -> bool {
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
