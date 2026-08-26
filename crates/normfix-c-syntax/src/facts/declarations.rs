//! Declaration, declarator, enum, and array facts.

use std::collections::HashMap;

use tree_sitter::Node;

use crate::parser::ParseFailure;

use super::model::{
    ArrayDeclaratorFact, DeclarationSplitFact, EnumConstantFact, SharedDeclarationFact,
    SyntaxFacts, UnusedLocalFact,
};
use super::nodes::{
    ancestor_kind, contains_kind, declarator_name, direct_named_children, has_ancestor_kind,
    innermost_identifier, node_range, node_text,
};

pub(super) fn enum_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<EnumConstantFact>, ParseFailure> {
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    let Some(enum_specifier) = ancestor_kind(node, "enum_specifier") else {
        return Ok(None);
    };
    let value = node.child_by_field_name("value");
    Ok(Some(EnumConstantFact {
        enum_range: node_range(enum_specifier.start_byte(), enum_specifier.end_byte())?,
        name: node_text(source, name)?.to_owned(),
        name_range: node_range(name.start_byte(), name.end_byte())?,
        explicit_value: value
            .map(|value| node_text(source, value).map(str::to_owned))
            .transpose()?,
        value_range: value
            .map(|value| node_range(value.start_byte(), value.end_byte()))
            .transpose()?,
    }))
}

pub(super) fn array_fact(
    source: &str,
    node: Node<'_>,
) -> Result<ArrayDeclaratorFact, ParseFailure> {
    let declared = node
        .child_by_field_name("declarator")
        .and_then(declarator_name);
    let size = node.child_by_field_name("size");
    Ok(ArrayDeclaratorFact {
        name: declared
            .map(|name| node_text(source, name).map(str::to_owned))
            .transpose()?,
        range: node_range(node.start_byte(), node.end_byte())?,
        bound: size
            .map(|size| node_text(source, size).map(str::to_owned))
            .transpose()?,
        bound_range: size
            .map(|size| node_range(size.start_byte(), size.end_byte()))
            .transpose()?,
    })
}

/// Records what a local declaration allows: splitting it, or deleting it.
pub(super) fn collect_declaration_facts(
    source: &str,
    node: Node<'_>,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    if let Some(fact) = declaration_split_fact(source, node)? {
        facts.declaration_splits.push(fact);
    }
    if let Some(name) = unused_local_name(source, node) {
        // Whether anything else mentions it is settled once, after the walk.
        facts.inert_declarations.push(UnusedLocalFact {
            range: node_range(node.start_byte(), node.end_byte())?,
            name,
        });
    }
    Ok(())
}

/// The name of a local that nothing reads and whose removal loses nothing.
///
/// Two separate things have to hold. The declaration must carry nothing that
/// runs: `int n = g();` is a call, and deleting it deletes the call, while a
/// `malloc` there would have its leak repaired by accident into a program the
/// reader did not write. And the name must appear exactly once in the file —
/// its own declaration — counted both in the tree and in the raw text, because
/// a macro body mentioning it is text the tree never shows.
fn unused_local_name(source: &str, node: Node<'_>) -> Option<String> {
    if !declaration_is_inert(node) {
        return None;
    }
    let mut cursor = node.walk();
    let declarators = node
        .children(&mut cursor)
        .filter(|child| matches!(child.kind(), "init_declarator" | "identifier"))
        .collect::<Vec<_>>();
    let [declarator] = declarators.as_slice() else {
        return None;
    };
    let target = if declarator.kind() == "identifier" {
        *declarator
    } else {
        innermost_identifier(declarator.child_by_field_name("declarator")?)?
    };
    let name = node_text(source, target).ok()?.to_owned();
    (!name.is_empty()).then_some(name)
}

/// Every word in the raw text, counted once.
///
/// Counting per candidate meant re-reading the whole file for each one, which
/// on an eight-hundred-line source was quadratic and showed up as a sevenfold
/// slowdown in the benchmark. One pass answers all of them.
pub(super) fn word_counts(source: &str) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    let mut start = None;
    for (index, character) in source.char_indices() {
        let is_part = character.is_alphanumeric() || character == '_';
        match (is_part, start) {
            (true, None) => start = Some(index),
            (false, Some(begin)) => {
                *counts.entry(&source[begin..index]).or_insert(0) += 1;
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        *counts.entry(&source[begin..]).or_insert(0) += 1;
    }
    counts
}

/// Whether deleting this declaration would delete nothing that runs.
///
/// A compiler saying a variable is unused is not on its own permission to
/// delete it: `-Wunused-variable` fires just as readily for `int n = g();` as
/// for `int n;`, and the first one carries a call. What makes the removal safe
/// is the initializer being something that cannot do anything — a literal, or
/// reading another variable — or there being no initializer at all. Anything
/// else, including a `malloc` whose removal would silently repair a leak into
/// a different program, is left alone.
fn declaration_is_inert(node: Node<'_>) -> bool {
    if !has_ancestor_kind(node, "function_definition") {
        return false;
    }
    let mut cursor = node.walk();
    let children = node.children(&mut cursor).collect::<Vec<_>>();
    if children
        .iter()
        .any(|child| matches!(child.kind(), "storage_class_specifier" | "ERROR"))
    {
        return false;
    }
    children.iter().all(|child| match child.kind() {
        "init_declarator" => child
            .child_by_field_name("value")
            .is_some_and(is_inert_expression),
        _ => true,
    })
}

/// Whether evaluating this expression can be skipped without losing anything.
fn is_inert_expression(node: Node<'_>) -> bool {
    match node.kind() {
        "number_literal"
        | "char_literal"
        | "true"
        | "false"
        | "null"
        | "identifier"
        | "string_literal"
        | "concatenated_string"
        | "sizeof_expression" => true,
        "parenthesized_expression" | "unary_expression" | "cast_expression" => {
            direct_named_children(node).all(is_inert_expression)
        }
        "binary_expression" => direct_named_children(node).all(is_inert_expression),
        _ => false,
    }
}

/// Reads a declaration that can be split from its initializer.
///
/// Everything this refuses is refused because the split would be a different
/// program, and the grammar is what tells them apart: a qualifier or storage
/// class is its own node, an aggregate initializer is its own node kind, and a
/// second declarator is a second child rather than something to guess at from
/// the text.
fn declaration_split_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<DeclarationSplitFact>, ParseFailure> {
    let mut cursor = node.walk();
    let children = node.children(&mut cursor).collect::<Vec<_>>();
    if children.iter().any(|child| {
        matches!(child.kind(), "storage_class_specifier" | "type_qualifier")
            || child.kind() == "ERROR"
    }) {
        return Ok(None);
    }
    let declarators = children
        .iter()
        .filter(|child| child.kind() == "init_declarator")
        .collect::<Vec<_>>();
    let [declarator] = declarators.as_slice() else {
        return Ok(None);
    };
    let (Some(target), Some(value)) = (
        declarator.child_by_field_name("declarator"),
        declarator.child_by_field_name("value"),
    ) else {
        return Ok(None);
    };
    // An aggregate initializer is initialization syntax, not an expression, and
    // an array cannot be assigned to at all.
    if value.kind() == "initializer_list" || target.kind() == "array_declarator" {
        return Ok(None);
    }
    let Some(name) = innermost_identifier(target) else {
        return Ok(None);
    };
    Ok(Some(DeclarationSplitFact {
        declaration_range: node_range(node.start_byte(), node.end_byte())?,
        strip_range: node_range(target.end_byte(), value.end_byte())?,
        name: node_text(source, name)?.to_owned(),
        value_range: node_range(value.start_byte(), value.end_byte())?,
    }))
}

/// A declaration naming several variables at once.
///
/// `int *a, b;` declares one pointer and one int, and that is the reason the
/// Norm asks for one per line: written out, nobody has to remember that the
/// star binds to the name and not to the type. So the split copies the
/// specifiers and keeps each declarator exactly as written, stars included,
/// rather than rebuilding a type it would have to guess at.
///
/// The declaration must be a plain one. A `typedef`, a struct or union body, or
/// anything the parser flagged is left alone, since the shared part is then not
/// a simple prefix that can be repeated.
pub(super) fn shared_declaration_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<SharedDeclarationFact>, ParseFailure> {
    // Almost every declaration names one thing, so the count and the
    // disqualifying kinds are settled in one pass that allocates nothing.
    let is_declarator = |kind: &str| {
        matches!(
            kind,
            "init_declarator"
                | "identifier"
                | "pointer_declarator"
                | "array_declarator"
                | "function_declarator"
        )
    };
    let mut cursor = node.walk();
    let mut count = 0_usize;
    let mut refused = false;
    for child in node.children(&mut cursor) {
        if is_declarator(child.kind()) {
            count += 1;
        } else if matches!(
            child.kind(),
            "ERROR" | "struct_specifier" | "union_specifier" | "enum_specifier"
        ) {
            refused = true;
        }
    }
    if count < 2 || refused || contains_kind(node, "comment") {
        return Ok(None);
    }
    let mut cursor = node.walk();
    let declarators = node
        .children(&mut cursor)
        .filter(|child| is_declarator(child.kind()))
        .collect::<Vec<_>>();
    let Some(first) = declarators.first() else {
        return Ok(None);
    };
    let specifiers = source
        .get(node.start_byte()..first.start_byte())
        .unwrap_or_default()
        .trim_end()
        .to_owned();
    if specifiers.is_empty() {
        return Ok(None);
    }
    Ok(Some(SharedDeclarationFact {
        range: node_range(node.start_byte(), node.end_byte())?,
        specifiers,
        declarators: declarators
            .iter()
            .map(|declarator| node_text(source, *declarator).map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?,
    }))
}
