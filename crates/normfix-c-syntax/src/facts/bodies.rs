//! Function-body, local-declaration, return, and control-body facts.

use tree_sitter::Node;

use crate::parser::ParseFailure;

use super::functions::single_declaration_name;
use super::model::{
    CFunctionFact, CStatementKind, InitialDeclarationBlockFact, LocalDeclarationFact,
    RedundantElseFact, ReturnFact, SingleStatementBodyFact, SyntaxFacts,
};
use super::nodes::{direct_named_children, node_range, node_text};

pub(super) fn collect_control_body_fact(
    body: Option<Node<'_>>,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    let Some(body) = body else {
        return Ok(());
    };
    if body.kind() != "compound_statement" {
        // A single-statement body still owns its own line. Recording it here
        // means the rule reads the tree instead of guessing where a condition
        // ended by scanning parentheses in the text.
        facts
            .control_inline_bodies
            .push(node_range(body.start_byte(), body.end_byte())?);
        return Ok(());
    }
    facts
        .control_compounds
        .push(node_range(body.start_byte(), body.end_byte())?);
    let children = direct_named_children(body).collect::<Vec<_>>();
    if children.len() != 1 {
        return Ok(());
    }
    let statement = children[0];
    let statement_kind = match statement.kind() {
        "expression_statement" => CStatementKind::Expression,
        "return_statement" => CStatementKind::Return,
        "break_statement" => CStatementKind::Break,
        "continue_statement" => CStatementKind::Continue,
        _ => return Ok(()),
    };
    facts.single_statement_bodies.push(SingleStatementBodyFact {
        compound_range: node_range(body.start_byte(), body.end_byte())?,
        statement_range: node_range(statement.start_byte(), statement.end_byte())?,
        statement_kind,
    });
    Ok(())
}

pub(super) fn redundant_else_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<RedundantElseFact>, ParseFailure> {
    let Some(consequence) = node.child_by_field_name("consequence") else {
        return Ok(None);
    };
    let Some(alternative) = node
        .child_by_field_name("alternative")
        .and_then(alternative_statement)
    else {
        return Ok(None);
    };
    if one_return(consequence).is_none() {
        return Ok(None);
    }
    let Some(return_statement) = one_return(alternative) else {
        return Ok(None);
    };
    let Some(gap) = source.get(consequence.end_byte()..alternative.start_byte()) else {
        return Err(ParseFailure::InvalidRange {
            start: consequence.end_byte(),
            end: alternative.start_byte(),
        });
    };
    let trimmed_start = gap.len().saturating_sub(gap.trim_start().len());
    let trimmed = gap.trim();
    if trimmed != "else" {
        return Ok(None);
    }
    let else_start = consequence.end_byte() + trimmed_start;
    Ok(Some(RedundantElseFact {
        else_keyword_range: node_range(else_start, else_start + 4)?,
        alternative_range: node_range(alternative.start_byte(), alternative.end_byte())?,
        return_range: node_range(return_statement.start_byte(), return_statement.end_byte())?,
    }))
}

pub(super) fn alternative_statement(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "else_clause" {
        direct_named_children(node).next()
    } else {
        Some(node)
    }
}

fn one_return(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "return_statement" {
        return Some(node);
    }
    if node.kind() != "compound_statement" {
        return None;
    }
    let children = direct_named_children(node).collect::<Vec<_>>();
    (children.len() == 1 && children[0].kind() == "return_statement").then_some(children[0])
}

pub(super) fn collect_function_body_facts(
    source: &str,
    definition: Node<'_>,
    function: &CFunctionFact,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    let Some(body) = definition.child_by_field_name("body") else {
        return Ok(());
    };
    let body_range = node_range(body.start_byte(), body.end_byte())?;
    let children = direct_named_children(body).collect::<Vec<_>>();
    let mut still_initial = true;
    let mut initial = Vec::new();
    let mut following_item = None;
    for child in children {
        if child.kind() == "declaration" {
            let name_node = single_declaration_name(child);
            let range = node_range(child.start_byte(), child.end_byte())?;
            facts.local_declarations.push(LocalDeclarationFact {
                function_name: function.name.clone(),
                range,
                name: name_node
                    .map(|name| node_text(source, name).map(str::to_owned))
                    .transpose()?,
                name_range: name_node
                    .map(|name| node_range(name.start_byte(), name.end_byte()))
                    .transpose()?,
                scope_range: body_range,
                initial: still_initial,
            });
            if still_initial {
                initial.push(range);
            }
        } else {
            if still_initial && !initial.is_empty() {
                following_item = Some(node_range(child.start_byte(), child.end_byte())?);
            }
            still_initial = false;
        }
    }
    if !initial.is_empty() {
        facts
            .initial_declaration_blocks
            .push(InitialDeclarationBlockFact {
                function_name: function.name.clone(),
                declarations: initial,
                following_item,
            });
    }
    collect_nested_local_declarations(source, body, function, facts)?;
    collect_returns(definition, function, facts)?;
    Ok(())
}

fn collect_nested_local_declarations(
    source: &str,
    function_body: Node<'_>,
    function: &CFunctionFact,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    let mut pending = direct_named_children(function_body).collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        if node.kind() == "compound_statement" {
            let scope_range = node_range(node.start_byte(), node.end_byte())?;
            for child in direct_named_children(node) {
                if child.kind() != "declaration" {
                    continue;
                }
                let range = node_range(child.start_byte(), child.end_byte())?;
                if facts
                    .local_declarations
                    .iter()
                    .any(|declaration| declaration.range == range)
                {
                    continue;
                }
                let name_node = single_declaration_name(child);
                facts.local_declarations.push(LocalDeclarationFact {
                    function_name: function.name.clone(),
                    range,
                    name: name_node
                        .map(|name| node_text(source, name).map(str::to_owned))
                        .transpose()?,
                    name_range: name_node
                        .map(|name| node_range(name.start_byte(), name.end_byte()))
                        .transpose()?,
                    scope_range,
                    initial: false,
                });
            }
        }
        let children = direct_named_children(node).collect::<Vec<_>>();
        pending.extend(children);
    }
    Ok(())
}

fn collect_returns(
    definition: Node<'_>,
    function: &CFunctionFact,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    let mut pending = vec![definition];
    while let Some(node) = pending.pop() {
        if node.kind() == "return_statement" {
            let expression = direct_named_children(node).next();
            facts.returns.push(ReturnFact {
                function_name: function.name.clone(),
                range: node_range(node.start_byte(), node.end_byte())?,
                expression_range: expression
                    .map(|expression| node_range(expression.start_byte(), expression.end_byte()))
                    .transpose()?,
                function_returns_pointer: function.returns_pointer,
            });
            continue;
        }
        let children = direct_named_children(node).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    Ok(())
}
