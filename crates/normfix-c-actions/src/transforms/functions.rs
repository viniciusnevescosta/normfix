use super::{
    BTreeSet, CActionError, CActionOptions, CFunctionKind, Edit, ParsedContext, ReportedDiagnostic,
    visual_width, visual_width_from,
};

#[derive(Clone, Copy, Debug)]
struct FunctionSignature {
    line: u32,
    declarator_start: usize,
    prefix_end: usize,
    definition: bool,
}

pub(super) fn fix_function_layout(
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

pub(super) fn next_tab_stop(column: u32) -> u32 {
    column.saturating_add(4 - ((column.saturating_sub(1)) % 4))
}

pub(super) fn tabs_to_column(mut column: u32, target: u32) -> Option<String> {
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
