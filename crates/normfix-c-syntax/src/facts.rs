//! Backend-neutral structural facts extracted during the single C parse.

use normfix_core::{TextRange, TextSize};
use tree_sitter::Node;

use crate::parser::ParseFailure;

/// Facts needed by native Norm rules without exposing Tree-sitter nodes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyntaxFacts {
    /// Function definitions and simple prototypes in source order.
    pub functions: Vec<CFunctionFact>,
    /// Enumerators in source order.
    pub enum_constants: Vec<EnumConstantFact>,
    /// Array declarators in source order.
    pub arrays: Vec<ArrayDeclaratorFact>,
    /// Preprocessor node ranges, including nested conditional regions.
    pub preprocessor_ranges: Vec<TextRange>,
}

/// Definition versus declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CFunctionKind {
    /// A function with a compound body.
    Definition,
    /// A declaration containing one simple function declarator.
    Prototype,
}

/// One backend-neutral function declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CFunctionFact {
    /// Declared identifier.
    pub name: String,
    /// Identifier byte range.
    pub name_range: TextRange,
    /// Complete definition/declaration byte range.
    pub range: TextRange,
    /// Range before the body/semicolon, including the declarator.
    pub signature_range: TextRange,
    /// Compound body for a definition.
    pub body_range: Option<TextRange>,
    /// Parameter-list range including parentheses.
    pub parameters_range: TextRange,
    /// Number of parameter declarations or variadic parameters.
    pub parameter_count: u32,
    /// Whether `static` occurs among definition/declaration specifiers.
    pub is_static: bool,
    /// Definition or prototype.
    pub kind: CFunctionKind,
}

/// One enum member and its optional explicit expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumConstantFact {
    /// Complete containing enum-specifier range.
    pub enum_range: TextRange,
    /// Enumerator identifier.
    pub name: String,
    /// Identifier range.
    pub name_range: TextRange,
    /// Explicit value expression as source text.
    pub explicit_value: Option<String>,
    /// Explicit expression range.
    pub value_range: Option<TextRange>,
}

/// One array declarator and its optional bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrayDeclaratorFact {
    /// Declared identifier when it can be recovered from the declarator.
    pub name: Option<String>,
    /// Complete array declarator range.
    pub range: TextRange,
    /// Bound expression text; `None` represents an incomplete array.
    pub bound: Option<String>,
    /// Bound expression range.
    pub bound_range: Option<TextRange>,
}

pub(crate) fn collect_facts(source: &str, root: Node<'_>) -> Result<SyntaxFacts, ParseFailure> {
    let mut facts = SyntaxFacts::default();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        match node.kind() {
            "function_definition" => {
                if let Some(fact) = function_definition_fact(source, node)? {
                    facts.functions.push(fact);
                }
            }
            "declaration" if !has_ancestor_kind(node, "function_definition") => {
                if let Some(fact) = prototype_fact(source, node)? {
                    facts.functions.push(fact);
                }
            }
            "enumerator" => {
                if let Some(fact) = enum_fact(source, node)? {
                    facts.enum_constants.push(fact);
                }
            }
            "array_declarator" => facts.arrays.push(array_fact(source, node)?),
            kind if kind.starts_with("preproc_") => {
                facts
                    .preprocessor_ranges
                    .push(node_range(node.start_byte(), node.end_byte())?);
            }
            _ => {}
        }
        let mut cursor = node.walk();
        let children = node.children(&mut cursor).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    facts.functions.sort_by_key(|fact| fact.range);
    facts.enum_constants.sort_by_key(|fact| fact.name_range);
    facts.arrays.sort_by_key(|fact| fact.range);
    facts.preprocessor_ranges.sort();
    facts.preprocessor_ranges.dedup();
    Ok(facts)
}

fn function_definition_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<CFunctionFact>, ParseFailure> {
    let Some(declarator) = node.child_by_field_name("declarator") else {
        return Ok(None);
    };
    let Some(function) = find_descendant_kind(declarator, "function_declarator") else {
        return Ok(None);
    };
    let Some(name_node) = declarator_name(function) else {
        return Ok(None);
    };
    let Some(parameters) = function.child_by_field_name("parameters") else {
        return Ok(None);
    };
    let Some(body) = node.child_by_field_name("body") else {
        return Ok(None);
    };
    let signature_range = node_range(node.start_byte(), body.start_byte())?;
    Ok(Some(CFunctionFact {
        name: node_text(source, name_node)?.to_owned(),
        name_range: node_range(name_node.start_byte(), name_node.end_byte())?,
        range: node_range(node.start_byte(), node.end_byte())?,
        signature_range,
        body_range: Some(node_range(body.start_byte(), body.end_byte())?),
        parameters_range: node_range(parameters.start_byte(), parameters.end_byte())?,
        parameter_count: parameter_count(parameters),
        is_static: direct_named_children(node).any(|child| {
            child.kind() == "storage_class_specifier"
                && node_text(source, child).is_ok_and(|text| text == "static")
        }),
        kind: CFunctionKind::Definition,
    }))
}

fn prototype_fact(source: &str, node: Node<'_>) -> Result<Option<CFunctionFact>, ParseFailure> {
    let function_nodes = descendants_kind(node, "function_declarator");
    if function_nodes.len() != 1 {
        return Ok(None);
    }
    let function = function_nodes[0];
    let Some(name_node) = declarator_name(function) else {
        return Ok(None);
    };
    let Some(parameters) = function.child_by_field_name("parameters") else {
        return Ok(None);
    };
    Ok(Some(CFunctionFact {
        name: node_text(source, name_node)?.to_owned(),
        name_range: node_range(name_node.start_byte(), name_node.end_byte())?,
        range: node_range(node.start_byte(), node.end_byte())?,
        signature_range: node_range(node.start_byte(), node.end_byte())?,
        body_range: None,
        parameters_range: node_range(parameters.start_byte(), parameters.end_byte())?,
        parameter_count: parameter_count(parameters),
        is_static: direct_named_children(node).any(|child| {
            child.kind() == "storage_class_specifier"
                && node_text(source, child).is_ok_and(|text| text == "static")
        }),
        kind: CFunctionKind::Prototype,
    }))
}

fn enum_fact(source: &str, node: Node<'_>) -> Result<Option<EnumConstantFact>, ParseFailure> {
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

fn array_fact(source: &str, node: Node<'_>) -> Result<ArrayDeclaratorFact, ParseFailure> {
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

fn parameter_count(parameters: Node<'_>) -> u32 {
    direct_named_children(parameters)
        .filter(|child| matches!(child.kind(), "parameter_declaration" | "variadic_parameter"))
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn declarator_name(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(node.kind(), "identifier" | "field_identifier") {
        return Some(node);
    }
    if let Some(declarator) = node.child_by_field_name("declarator") {
        return declarator_name(declarator);
    }
    direct_named_children(node).find_map(declarator_name)
}

fn find_descendant_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    direct_named_children(node).find_map(|child| find_descendant_kind(child, kind))
}

fn descendants_kind<'tree>(node: Node<'tree>, kind: &str) -> Vec<Node<'tree>> {
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

fn direct_named_children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .collect::<Vec<_>>()
        .into_iter()
}

fn has_ancestor_kind(mut node: Node<'_>, kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return true;
        }
        node = parent;
    }
    false
}

fn ancestor_kind<'tree>(mut node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn node_text<'source>(source: &'source str, node: Node<'_>) -> Result<&'source str, ParseFailure> {
    source
        .get(node.start_byte()..node.end_byte())
        .ok_or(ParseFailure::InvalidRange {
            start: node.start_byte(),
            end: node.end_byte(),
        })
}

fn node_range(start: usize, end: usize) -> Result<TextRange, ParseFailure> {
    let start_size =
        TextSize::try_from(start).map_err(|_| ParseFailure::InvalidRange { start, end })?;
    let end_size =
        TextSize::try_from(end).map_err(|_| ParseFailure::InvalidRange { start, end })?;
    TextRange::new(start_size, end_size).ok_or(ParseFailure::InvalidRange { start, end })
}
