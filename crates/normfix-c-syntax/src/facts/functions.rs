//! Function declarations, definitions, and parameter facts.

use tree_sitter::Node;

use crate::parser::ParseFailure;

use super::model::{CFunctionFact, CFunctionKind, CParameterFact};
use super::nodes::{
    declarator_name, descendants_kind, direct_named_children, find_descendant_kind,
    has_ancestor_kind, node_range, node_text,
};

pub(super) fn function_definition_fact(
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
        parameter_count: parameter_count(source, parameters),
        parameters: parameter_facts(source, parameters)?,
        is_static: direct_named_children(node).any(|child| {
            child.kind() == "storage_class_specifier"
                && node_text(source, child).is_ok_and(|text| text == "static")
        }),
        returns_pointer: declarator_returns_pointer(declarator, function),
        kind: CFunctionKind::Definition,
    }))
}

pub(super) fn prototype_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<CFunctionFact>, ParseFailure> {
    // A function typedef and a function-pointer typedef both contain a
    // `function_declarator` in Tree-sitter's C grammar, but neither declares a
    // callable function symbol. Keep this exclusion at the fact boundary so no
    // downstream consumer can mistake a type alias for a prototype.
    if declaration_is_typedef(source, node)? {
        return Ok(None);
    }
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
        parameter_count: parameter_count(source, parameters),
        parameters: parameter_facts(source, parameters)?,
        is_static: direct_named_children(node).any(|child| {
            child.kind() == "storage_class_specifier"
                && node_text(source, child).is_ok_and(|text| text == "static")
        }),
        returns_pointer: declarator_returns_pointer(node, function),
        kind: CFunctionKind::Prototype,
    }))
}

fn declaration_is_typedef(source: &str, node: Node<'_>) -> Result<bool, ParseFailure> {
    if node.kind() == "type_definition" || has_ancestor_kind(node, "type_definition") {
        return Ok(true);
    }
    for child in direct_named_children(node) {
        if child.kind() == "storage_class_specifier" && node_text(source, child)? == "typedef" {
            return Ok(true);
        }
    }
    Ok(false)
}

fn declarator_returns_pointer(declarator: Node<'_>, function: Node<'_>) -> bool {
    let mut current = function;
    while let Some(parent) = current.parent() {
        if parent.kind() == "pointer_declarator" {
            return true;
        }
        if parent.id() == declarator.id() {
            break;
        }
        current = parent;
    }
    false
}

fn parameter_count(source: &str, parameters: Node<'_>) -> u32 {
    let declarations = direct_named_children(parameters)
        .filter(|child| matches!(child.kind(), "parameter_declaration" | "variadic_parameter"))
        .collect::<Vec<_>>();
    if declarations.len() == 1
        && declarations[0].kind() == "parameter_declaration"
        && declarations[0].child_by_field_name("declarator").is_none()
        && source
            .get(declarations[0].byte_range())
            .is_some_and(|text| text.trim() == "void")
    {
        return 0;
    }
    declarations.len().try_into().unwrap_or(u32::MAX)
}

pub(super) fn single_declaration_name(declaration: Node<'_>) -> Option<Node<'_>> {
    let declarators = (0..declaration.child_count())
        .filter(|index| {
            u32::try_from(*index)
                .ok()
                .and_then(|index| declaration.field_name_for_child(index))
                == Some("declarator")
        })
        .filter_map(|index| {
            u32::try_from(index)
                .ok()
                .and_then(|index| declaration.child(index))
        })
        .collect::<Vec<_>>();
    (declarators.len() == 1)
        .then(|| declarator_name(declarators[0]))
        .flatten()
}

fn parameter_facts(
    source: &str,
    parameters: Node<'_>,
) -> Result<Vec<CParameterFact>, ParseFailure> {
    direct_named_children(parameters)
        .filter(|parameter| parameter.kind() == "parameter_declaration")
        .filter_map(|parameter| {
            parameter
                .child_by_field_name("declarator")
                .and_then(declarator_name)
        })
        .map(|name| {
            Ok(CParameterFact {
                name: node_text(source, name)?.to_owned(),
                name_range: node_range(name.start_byte(), name.end_byte())?,
            })
        })
        .collect()
}
