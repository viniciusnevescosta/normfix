//! Conservative expression-statement rewrite candidates.

use tree_sitter::Node;

use crate::parser::ParseFailure;

use super::model::{ChainedAssignmentFact, TernaryFact, TernaryForm};
use super::nodes::{contains_kind, node_range, node_text, unwrap_parentheses};

/// A statement whose whole value is one `?:`, when rewriting it is provably the
/// same program.
///
/// The Norm forbids the operator, so every one of these has to go; the guards
/// decide which ones normfix may retire itself instead of handing back.
///
/// The statement must sit directly in a block. The same rewrite under an
/// unbraced `if` body would put an `if` where one statement was, and the
/// trailing `else` would bind to the wrong branch — a silent change of meaning.
///
/// It must carry no comment, which the new shape has nowhere to put.
///
/// An assignment target is written into both branches, so it must hold nothing
/// that runs: a call or an increment there happens once today and would have to
/// happen once still, which only a side-effect-free target guarantees.
///
/// And no part may hold a second `?:`. That one would end up inside a branch,
/// where this collector no longer looks, and the run would report a ternary
/// removed while the file still had one.
pub(super) fn ternary_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<TernaryFact>, ParseFailure> {
    // Ordered cheapest first. This runs for every statement in the file, and
    // almost none of them hold a `?:`, so nothing that walks a subtree may
    // happen before the field lookups that rule the statement out.
    if node
        .parent()
        .is_none_or(|parent| parent.kind() != "compound_statement")
    {
        return Ok(None);
    }
    let Some(value) = node.named_child(0) else {
        return Ok(None);
    };
    let assignment = (node.kind() == "expression_statement").then_some(value);
    let conditional = match assignment {
        None => unwrap_parentheses(value),
        Some(assignment) => {
            if assignment.kind() != "assignment_expression" {
                return Ok(None);
            }
            let Some(right) = assignment.child_by_field_name("right") else {
                return Ok(None);
            };
            unwrap_parentheses(right)
        }
    };
    // The node kind settles it for almost every statement in the file, and
    // costs one comparison; naming a field costs a scan of the children.
    if conditional.kind() != "conditional_expression" {
        return Ok(None);
    }
    let (Some(condition), Some(consequence), Some(alternative)) = (
        conditional.child_by_field_name("condition"),
        conditional.child_by_field_name("consequence"),
        conditional.child_by_field_name("alternative"),
    ) else {
        return Ok(None);
    };
    // Everything below walks subtrees or allocates, and from here on the
    // statement is known to hold a ternary.
    if contains_kind(node, "comment")
        || [condition, consequence, alternative]
            .iter()
            .any(|part| contains_kind(*part, "conditional_expression"))
    {
        return Ok(None);
    }
    let form = match assignment {
        None => TernaryForm::Return,
        Some(assignment) => {
            let (Some(target), Some(operator)) = (
                assignment.child_by_field_name("left"),
                assignment.child_by_field_name("operator"),
            ) else {
                return Ok(None);
            };
            if [
                "call_expression",
                "update_expression",
                "assignment_expression",
            ]
            .iter()
            .any(|kind| contains_kind(target, kind))
            {
                return Ok(None);
            }
            TernaryForm::Assignment {
                target: node_text(source, target)?.to_owned(),
                operator: node_text(source, operator)?.to_owned(),
            }
        }
    };
    let mut owner = node;
    while owner.kind() != "function_definition" {
        let Some(parent) = owner.parent() else {
            return Ok(None);
        };
        owner = parent;
    }
    let Some(body) = owner.child_by_field_name("body") else {
        return Ok(None);
    };
    Ok(Some(TernaryFact {
        statement_range: node_range(node.start_byte(), node.end_byte())?,
        form,
        condition_range: node_range(condition.start_byte(), condition.end_byte())?,
        consequence_range: node_range(consequence.start_byte(), consequence.end_byte())?,
        alternative_range: node_range(alternative.start_byte(), alternative.end_byte())?,
        condition_parenthesized: condition.kind() == "parenthesized_expression",
        function_body_range: node_range(body.start_byte(), body.end_byte())?,
    }))
}

/// A statement assigning one value to two names at once.
///
/// `a = b = 0;` is `b = 0` first, and then `a` takes the value `b` holds after
/// it — after any conversion `b`'s type imposes, which is why the second
/// statement reads `b` rather than repeating the value. Written out, that is
/// `b = 0;` and `a = b;`, in that order, and any call on the right still
/// happens exactly once.
///
/// The targets are read as well as written now, so neither may do anything
/// itself; and a longer chain is left to the next pass, where the inner
/// assignment is a statement of this same shape.
pub(super) fn chained_assignment_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<ChainedAssignmentFact>, ParseFailure> {
    if node
        .parent()
        .is_none_or(|parent| parent.kind() != "compound_statement")
    {
        return Ok(None);
    }
    let Some(outer) = node
        .named_child(0)
        .filter(|child| child.kind() == "assignment_expression")
    else {
        return Ok(None);
    };
    // The right-hand side settles it for every ordinary assignment in the
    // file, so it is named before the two fields only a chain needs.
    let Some(right) = outer.child_by_field_name("right") else {
        return Ok(None);
    };
    let inner = unwrap_parentheses(right);
    if inner.kind() != "assignment_expression" {
        return Ok(None);
    }
    let (Some(target), Some(operator), Some(inner_target)) = (
        outer.child_by_field_name("left"),
        outer.child_by_field_name("operator"),
        inner.child_by_field_name("left"),
    ) else {
        return Ok(None);
    };
    if contains_kind(node, "comment")
        || [
            "call_expression",
            "update_expression",
            "assignment_expression",
        ]
        .iter()
        .any(|kind| contains_kind(target, kind) || contains_kind(inner_target, kind))
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
    let Some(body) = owner.child_by_field_name("body") else {
        return Ok(None);
    };
    Ok(Some(ChainedAssignmentFact {
        statement_range: node_range(node.start_byte(), node.end_byte())?,
        target: node_text(source, target)?.to_owned(),
        operator: node_text(source, operator)?.to_owned(),
        inner: node_text(source, inner)?.to_owned(),
        inner_target: node_text(source, inner_target)?.to_owned(),
        function_body_range: node_range(body.start_byte(), body.end_byte())?,
    }))
}
