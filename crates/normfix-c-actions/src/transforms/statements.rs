use super::{
    CActionError, Edit, HashMap, LexicalMap, ParsedContext, SourceLines, TernaryForm, TextRange,
    leading_whitespace,
};

pub(super) fn remove_single_statement_braces(
    context: &ParsedContext,
) -> Result<Vec<Edit>, CActionError> {
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
pub(super) fn remove_empty_statements(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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
pub(super) fn remove_unused_variables(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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
pub(super) fn split_declarations(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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
pub(super) fn split_chained_assignments(
    context: &ParsedContext,
) -> Result<Vec<Edit>, CActionError> {
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
pub(super) fn split_shared_declarations(
    context: &ParsedContext,
) -> Result<Vec<Edit>, CActionError> {
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
        if !text[..relative].trim().is_empty()
            || !text[end.saturating_sub(line.start)..].trim().is_empty()
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
pub(super) fn separate_crowded_statements(
    context: &ParsedContext,
) -> Result<Vec<Edit>, CActionError> {
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
pub(super) fn rewrite_for_loops(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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
        let text =
            |range: Option<TextRange>| range.and_then(|range| source.get(range_bounds(range)));
        let (initializer, step) = (
            text(loop_fact.initializer_range),
            text(loop_fact.step_range),
        );
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
        if lines.get(closing).is_some_and(|last| {
            !lines.text(last)[end.saturating_sub(last.start)..]
                .trim()
                .is_empty()
        }) {
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
pub(super) fn rewrite_ternaries(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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

pub(super) fn remove_redundant_else(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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

pub(super) fn find_brace_near(
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

pub(super) fn control_condition_close(
    text: &str,
    base: usize,
    lexical: &LexicalMap,
) -> Option<usize> {
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
