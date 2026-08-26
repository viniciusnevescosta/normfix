use super::{
    BTreeMap, BTreeSet, CActionError, Edit, ParsedContext, ReportedDiagnostic, is_identifier,
    leading_whitespace, preprocessor_line_set, visual_width, visual_width_from, whitespace_before,
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct IndentInfo {
    pub(super) expected: u32,
    pub(super) brace_depth: u32,
    pub(super) delimiter_depth: u32,
    pub(super) continuation_extra: u32,
    pub(super) continuation: bool,
}

pub(super) fn indentation_model(context: &ParsedContext) -> BTreeMap<u32, IndentInfo> {
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
pub(super) fn is_control_header(code: &str) -> bool {
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
pub(super) fn fix_indentation(
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
    for line_number in diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "SPACE_REPLACE_TAB")
        .map(|diagnostic| diagnostic.line)
        .collect::<BTreeSet<_>>()
    {
        let Some((start, end)) = composite_typedef_alias_gap(context, line_number) else {
            continue;
        };
        edits.push(Edit::new(
            start,
            end,
            "\t",
            "SPACE_REPLACE_TAB",
            "replaced the space before a composite typedef alias with the required tab",
            Some(line_number),
        )?);
    }
    Ok(edits)
}

/// Finds the whitespace in the closing line of a simple composite typedef.
///
/// Tree-sitter's token stream supplies the proof rather than a text pattern:
/// the physical line must contain exactly `}`, one identifier and `;`; the
/// brace must match an opening brace introduced by `typedef struct`, `union`,
/// or `enum`. This deliberately leaves declarations with attributes, macros,
/// comments, or any other decoration for review.
fn composite_typedef_alias_gap(
    context: &ParsedContext,
    line_number: u32,
) -> Option<(usize, usize)> {
    let line = context.lines().get(line_number)?;
    let indexed = context
        .tokens()
        .iter()
        .enumerate()
        .filter(|(_, token)| token.start >= line.start && token.end <= line.content_end)
        .collect::<Vec<_>>();
    let [(closing_index, closing), (_, alias), (_, semicolon)] = indexed.as_slice() else {
        return None;
    };
    if closing.text != "}"
        || !is_identifier(&alias.text)
        || semicolon.text != ";"
        || closing.end > alias.start
    {
        return None;
    }
    let gap = context.source().get(closing.end..alias.start)?;
    if !gap.contains(' ') || !gap.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
        return None;
    }

    let tokens = context.tokens();
    let mut depth = 1_u32;
    let mut opening_index = None;
    for index in (0..*closing_index).rev() {
        match tokens[index].text.as_str() {
            "}" => depth = depth.saturating_add(1),
            "{" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    opening_index = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let opening_index = opening_index?;
    let declaration_prefix = tokens[..opening_index]
        .iter()
        .rev()
        .take_while(|token| !matches!(token.text.as_str(), ";" | "{" | "}"))
        .map(|token| token.text.as_str())
        .collect::<BTreeSet<_>>();
    if !declaration_prefix.contains("typedef")
        || !["struct", "union", "enum"]
            .iter()
            .any(|keyword| declaration_prefix.contains(keyword))
    {
        return None;
    }
    Some((closing.end, alias.start))
}

pub(super) fn whitespace_run_near(text: &str, index: usize) -> (usize, usize) {
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
