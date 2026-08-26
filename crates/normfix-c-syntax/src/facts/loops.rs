//! Loop classification and conservative loop-rewrite facts.

use tree_sitter::Node;

use crate::parser::ParseFailure;

use super::model::{ForLoopFact, LoopFact};
use super::nodes::{contains_kind, direct_named_children, node_range, node_text};

pub(super) fn loop_fact(source: &str, node: Node<'_>) -> Result<LoopFact, ParseFailure> {
    let condition = node.child_by_field_name("condition");
    let unconditional = match node.kind() {
        "for_statement" => condition
            .map(|condition| node_text(source, condition).map(literal_true))
            .transpose()?
            .unwrap_or(true),
        "while_statement" | "do_statement" => condition
            .map(|condition| node_text(source, condition).map(literal_true))
            .transpose()?
            .unwrap_or(false),
        _ => false,
    };
    let body = node.child_by_field_name("body");
    Ok(LoopFact {
        range: node_range(node.start_byte(), node.end_byte())?,
        unconditional,
        has_obvious_exit: body.is_some_and(|body| has_loop_exit(body, node)),
    })
}

fn literal_true(text: &str) -> bool {
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '(' | ')'))
        .collect::<String>();
    matches!(compact.as_str(), "1" | "true")
}

fn has_loop_exit(body: Node<'_>, owner: Node<'_>) -> bool {
    let mut pending = vec![body];
    while let Some(node) = pending.pop() {
        if matches!(node.kind(), "return_statement" | "goto_statement") {
            return true;
        }
        if node.kind() == "break_statement" && break_exits_owner(node, owner) {
            return true;
        }
        let children = direct_named_children(node).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    false
}

fn break_exits_owner(mut node: Node<'_>, owner: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "while_statement" | "for_statement" | "do_statement" | "switch_statement"
        ) {
            return parent.id() == owner.id();
        }
        node = parent;
    }
    false
}

/// A `for` loop a `while` can say exactly.
///
/// The Norm forbids `for` outright, and the pieces map across: the initializer
/// runs once before the loop, the condition is the `while` condition, and the
/// step goes last in the body, where it still runs after every iteration.
///
/// That last part is the whole proof, and it is what the guards protect. A
/// `continue` bound to this loop jumps to the step in a `for` and past it in a
/// `while`, which is a different loop and usually an endless one. A declaration
/// in the initializer is scoped to the loop, and moving it out widens that
/// scope. The loop must sit directly in a block, since one statement becomes
/// two and an unbraced body above it would keep only the first.
pub(super) fn for_loop_fact(node: Node<'_>) -> Result<Option<ForLoopFact>, ParseFailure> {
    let (Some(body), Some(parent)) = (node.child_by_field_name("body"), node.parent()) else {
        return Ok(None);
    };
    let initializer = node.child_by_field_name("initializer");
    if parent.kind() != "compound_statement"
        || initializer.is_some_and(|init| init.kind() == "declaration")
        || contains_kind(node, "comment")
        || jumps_to_the_step(body)
    {
        return Ok(None);
    }
    let mut owner = node;
    while owner.kind() != "function_definition" {
        let Some(next) = owner.parent() else {
            return Ok(None);
        };
        owner = next;
    }
    let Some(function_body) = owner.child_by_field_name("body") else {
        return Ok(None);
    };
    let range = |part: Option<Node<'_>>| {
        part.map(|part| node_range(part.start_byte(), part.end_byte()))
            .transpose()
    };
    Ok(Some(ForLoopFact {
        statement_range: node_range(node.start_byte(), node.end_byte())?,
        initializer_range: range(initializer)?,
        condition_range: range(node.child_by_field_name("condition"))?,
        step_range: range(node.child_by_field_name("update"))?,
        body_range: node_range(body.start_byte(), body.end_byte())?,
        body_is_block: body.kind() == "compound_statement",
        function_body_range: node_range(function_body.start_byte(), function_body.end_byte())?,
    }))
}

/// Whether `continue` inside this body would skip a step moved to the end.
///
/// A `continue` in a loop nested inside the body belongs to that loop and never
/// reaches this one, so the walk stops where a new loop begins.
fn jumps_to_the_step(node: Node<'_>) -> bool {
    if node.kind() == "continue_statement" {
        return true;
    }
    if matches!(
        node.kind(),
        "for_statement" | "while_statement" | "do_statement"
    ) {
        return false;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).any(jumps_to_the_step)
}
