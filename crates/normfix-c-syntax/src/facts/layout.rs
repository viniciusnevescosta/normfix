//! Layout facts derived directly from syntax-node boundaries.

use tree_sitter::Node;

use crate::parser::ParseFailure;

use super::model::SyntaxFacts;
use super::nodes::{direct_named_children, node_range};

/// Records the `*` that binds a declarator to its name.
///
/// The grammar tells this star apart from multiplication by node kind rather
/// than by guessing from the surrounding spaces.
pub(super) fn collect_pointer_star(
    node: Node<'_>,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    let mut cursor = node.walk();
    let star = node
        .children(&mut cursor)
        .find(|child| !child.is_named() && child.kind() == "*");
    if let Some(star) = star {
        facts
            .pointer_stars
            .push(node_range(star.start_byte(), star.end_byte())?);
    }
    Ok(())
}

/// Records an operator that has an operand on each side.
///
/// The grammar names the operator of a binary or assignment expression, and
/// gives unary `-a` and `a++` their own node kinds. That distinction is the
/// whole proof: only an operator with an operand on each side takes a space on
/// each side, and reading it from the tree cannot confuse the two the way a
/// text scan can.
pub(super) fn collect_binary_operator(
    node: Node<'_>,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    if let Some(operator) = node.child_by_field_name("operator") {
        facts
            .binary_operators
            .push(node_range(operator.start_byte(), operator.end_byte())?);
    }
    Ok(())
}

/// Whether this statement is a lone `;` that can be deleted.
///
/// A bare `;` parses as an expression statement with no expression: valid C
/// that executes nothing. Two conditions carry the proof that deleting it
/// changes nothing. First, the parent must be a block or the file itself: the
/// same node shape is also how `while (x);` and `for (;;);` spell an empty
/// body, and deleting one of those would silently promote the next statement
/// into the loop. Second, no preprocessor directive may sit immediately before
/// it, because then the `;` may terminate a statement that exists in only one
/// build configuration, which this parse cannot see.
pub(super) fn is_stray_semicolon(node: Node<'_>) -> bool {
    direct_named_children(node).next().is_none()
        && node.parent().is_some_and(|parent| {
            matches!(parent.kind(), "compound_statement" | "translation_unit")
        })
        && !node
            .prev_sibling()
            .is_some_and(|sibling| sibling.kind().starts_with("preproc"))
}

/// The gaps between instructions that share a physical line.
///
/// The Norm allows one instruction or control structure per line, and the fix
/// is a newline in a gap that holds none — no token moves, none is added, and
/// none is taken away.
///
/// The gap is recorded rather than the statement, because what has to change is
/// exactly what sits between the two: a directive there belongs to a build
/// configuration this parse cannot see, and a comment there would have to
/// choose a line, so both leave the pair alone.
pub(super) fn collect_crowded_statements(
    node: Node<'_>,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    let mut cursor = node.walk();
    let mut previous: Option<Node<'_>> = None;
    for child in node.named_children(&mut cursor) {
        let Some(last) = previous.replace(child) else {
            continue;
        };
        if child.kind() == "comment"
            || last.kind() == "comment"
            || child.kind().starts_with("preproc_")
            || last.kind().starts_with("preproc_")
            || last.end_position().row != child.start_position().row
        {
            continue;
        }
        facts
            .crowded_statements
            .push(node_range(last.end_byte(), child.start_byte())?);
    }
    Ok(())
}
