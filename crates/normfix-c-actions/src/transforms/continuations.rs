use super::{
    BTreeSet, CActionError, Edit, ParsedContext, has_sensitive_line_end, is_control_header,
    is_declaration_word, is_identifier, leading_whitespace, preprocessor_line_set, visual_width,
};

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

pub(super) fn compact_continuations(
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
