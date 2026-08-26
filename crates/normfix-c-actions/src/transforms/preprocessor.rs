use super::{
    BTreeMap, BTreeSet, CActionError, Edit, IncludeOrderKey, ParsedContext, PhysicalLine,
    ReportedDiagnostic, SourceLines, escaped_physical_newline, function_infos, include_order_key,
    leading_whitespace, multiline_preprocessor_lines,
};

/// Reorders each contiguous include block: system headers first, then project
/// headers, alphabetically inside both categories.
///
/// A block ends at the first line that is not exactly one include directive, so
/// a comment, blank line, conditional, macro definition, or trailing text keeps
/// the surrounding directives where they are. That containment is the proof:
/// nothing is moved across a construct that could change what a header means.
pub(super) fn reorder_includes(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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

pub(super) fn format_preprocessors(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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

pub(super) fn preprocessor_line_set(source: &str, lines: &SourceLines<'_>) -> BTreeSet<u32> {
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

pub(super) fn has_sensitive_line_end(text: &str) -> bool {
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

pub(super) fn remove_invalid_comments(
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
