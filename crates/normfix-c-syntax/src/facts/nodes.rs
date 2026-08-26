//! Small, shared Tree-sitter traversal and range helpers.

use normfix_core::{TextRange, TextSize};
use tree_sitter::Node;

use crate::parser::ParseFailure;

/// The identifier a declarator finally names, past any pointer stars.
pub(super) fn innermost_identifier(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node;
    loop {
        match current.kind() {
            "identifier" => return Some(current),
            "pointer_declarator" | "parenthesized_declarator" => {
                current = current.child_by_field_name("declarator")?;
            }
            _ => return None,
        }
    }
}

pub(super) fn declarator_name(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(node.kind(), "identifier" | "field_identifier") {
        return Some(node);
    }
    if let Some(declarator) = node.child_by_field_name("declarator") {
        return declarator_name(declarator);
    }
    direct_named_children(node).find_map(declarator_name)
}

pub(super) fn find_descendant_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    direct_named_children(node).find_map(|child| find_descendant_kind(child, kind))
}

pub(super) fn descendants_kind<'tree>(node: Node<'tree>, kind: &str) -> Vec<Node<'tree>> {
    let mut matches = Vec::new();
    let mut pending = vec![node];
    while let Some(candidate) = pending.pop() {
        if candidate.kind() == kind {
            matches.push(candidate);
            continue;
        }
        let children = direct_named_children(candidate).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    matches
}

pub(super) fn direct_named_children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = node.walk();
    let mut started = false;
    std::iter::from_fn(move || {
        loop {
            let advanced = if started {
                cursor.goto_next_sibling()
            } else {
                started = true;
                cursor.goto_first_child()
            };
            if !advanced {
                return None;
            }
            let child = cursor.node();
            if child.is_named() {
                return Some(child);
            }
        }
    })
}

/// Peels the parentheses a value was written inside, which change no meaning.
pub(super) fn unwrap_parentheses(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == "parenthesized_expression" {
        let Some(inner) = node.named_child(0) else {
            return node;
        };
        node = inner;
    }
    node
}

/// Whether `kind` appears anywhere at or below `node`.
pub(super) fn contains_kind(node: Node<'_>, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| contains_kind(child, kind))
}

pub(super) fn has_ancestor_kind(mut node: Node<'_>, kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return true;
        }
        node = parent;
    }
    false
}

pub(super) fn ancestor_kind<'tree>(mut node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

pub(super) fn node_text<'source>(
    source: &'source str,
    node: Node<'_>,
) -> Result<&'source str, ParseFailure> {
    source
        .get(node.start_byte()..node.end_byte())
        .ok_or(ParseFailure::InvalidRange {
            start: node.start_byte(),
            end: node.end_byte(),
        })
}

pub(super) fn node_range(start: usize, end: usize) -> Result<TextRange, ParseFailure> {
    let start_size =
        TextSize::try_from(start).map_err(|_| ParseFailure::InvalidRange { start, end })?;
    let end_size =
        TextSize::try_from(end).map_err(|_| ParseFailure::InvalidRange { start, end })?;
    TextRange::new(start_size, end_size).ok_or(ParseFailure::InvalidRange { start, end })
}
