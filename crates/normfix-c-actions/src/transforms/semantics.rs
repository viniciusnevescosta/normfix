use super::{
    BTreeSet, CActionError, CFunctionKind, Edit, ParsedContext, ReportedDiagnostic, Token,
    matching_forward, preprocessor_line_set,
};

pub(super) fn replace_pointer_zero_returns(
    context: &ParsedContext,
) -> Result<Vec<Edit>, CActionError> {
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

pub(super) fn compact_null_checks(context: &ParsedContext) -> Result<Vec<Edit>, CActionError> {
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

pub(super) fn parenthesize_returns(
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

pub(super) fn add_void_to_definitions(
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
