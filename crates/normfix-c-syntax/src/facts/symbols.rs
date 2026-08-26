//! Null, type-tag, call, and preprocessor symbol facts.

use tree_sitter::Node;

use crate::parser::ParseFailure;

use super::model::{CTypeTagKind, CallFact, MacroFact, NullCheckFact, TypeTagFact};
use super::nodes::{node_range, node_text};

pub(super) fn null_check_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<NullCheckFact>, ParseFailure> {
    let Some(left) = node.child_by_field_name("left") else {
        return Ok(None);
    };
    let Some(right) = node.child_by_field_name("right") else {
        return Ok(None);
    };
    let operator = source
        .get(left.end_byte()..right.start_byte())
        .ok_or(ParseFailure::InvalidRange {
            start: left.end_byte(),
            end: right.start_byte(),
        })?
        .trim();
    if !matches!(operator, "==" | "!=") {
        return Ok(None);
    }
    let left_text = node_text(source, left)?;
    let right_text = node_text(source, right)?;
    let operand = if left.kind() == "identifier" && right_text == "NULL" && left_text != "NULL" {
        left_text
    } else if right.kind() == "identifier" && left_text == "NULL" && right_text != "NULL" {
        right_text
    } else {
        return Ok(None);
    };
    Ok(Some(NullCheckFact {
        range: node_range(node.start_byte(), node.end_byte())?,
        operand: operand.to_owned(),
        equals: operator == "==",
    }))
}

pub(super) fn null_provider(source: &str, node: Node<'_>) -> Result<bool, ParseFailure> {
    let text = node_text(source, node)?;
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if node.kind() == "preproc_include" && compact == "#include <stddef.h>" {
        return Ok(true);
    }
    if node.kind() != "preproc_def" {
        return Ok(false);
    }
    let Some(value) = compact.strip_prefix("#define NULL ") else {
        return Ok(false);
    };
    let value = value.replace(' ', "");
    Ok(matches!(
        value.as_str(),
        "0" | "0L" | "(void*)0" | "((void*)0)"
    ))
}

pub(super) fn null_invalidator(source: &str, node: Node<'_>) -> Result<bool, ParseFailure> {
    let text = node_text(source, node)?;
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(compact == "#undef NULL"
        || ((node.kind() == "preproc_def" || node.kind() == "preproc_function_def")
            && compact.starts_with("#define NULL ")))
}

pub(super) fn type_tag_fact(
    source: &str,
    node: Node<'_>,
) -> Result<Option<TypeTagFact>, ParseFailure> {
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    let kind = match node.kind() {
        "struct_specifier" => CTypeTagKind::Struct,
        "union_specifier" => CTypeTagKind::Union,
        "enum_specifier" => CTypeTagKind::Enum,
        _ => return Ok(None),
    };
    Ok(Some(TypeTagFact {
        kind,
        name: node_text(source, name)?.to_owned(),
        name_range: node_range(name.start_byte(), name.end_byte())?,
    }))
}

pub(super) fn call_fact(source: &str, node: Node<'_>) -> Result<Option<CallFact>, ParseFailure> {
    let Some(callee) = node.child_by_field_name("function") else {
        return Ok(None);
    };
    if callee.kind() != "identifier" {
        return Ok(None);
    }
    Ok(Some(CallFact {
        name: node_text(source, callee)?.to_owned(),
        name_range: node_range(callee.start_byte(), callee.end_byte())?,
    }))
}

pub(super) fn macro_fact(source: &str, node: Node<'_>) -> Result<Option<MacroFact>, ParseFailure> {
    if !matches!(node.kind(), "preproc_def" | "preproc_function_def") {
        return Ok(None);
    }
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    Ok(Some(MacroFact {
        name: node_text(source, name)?.to_owned(),
        name_range: node_range(name.start_byte(), name.end_byte())?,
        function_like: node.kind() == "preproc_function_def",
    }))
}
