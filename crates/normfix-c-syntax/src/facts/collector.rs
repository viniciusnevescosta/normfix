//! Single-pass Tree-sitter dispatch and deterministic fact ordering.

use tree_sitter::Node;

use crate::parser::ParseFailure;

use super::bodies::{
    alternative_statement, collect_control_body_fact, collect_function_body_facts,
    redundant_else_fact,
};
use super::conditionals::{chained_assignment_fact, ternary_fact};
use super::declarations::{
    array_fact, collect_declaration_facts, enum_fact, shared_declaration_fact, word_counts,
};
use super::functions::{function_definition_fact, prototype_fact};
use super::layout::{
    collect_binary_operator, collect_crowded_statements, collect_pointer_star, is_stray_semicolon,
};
use super::loops::{for_loop_fact, loop_fact};
use super::model::SyntaxFacts;
use super::nodes::{has_ancestor_kind, node_range};
use super::symbols::{
    call_fact, macro_fact, null_check_fact, null_invalidator, null_provider, type_tag_fact,
};

// One arm per node kind the Norm has something to say about: the dispatch is a
// table, and breaking it up would scatter the walk it belongs to.
#[allow(clippy::too_many_lines)]
pub(crate) fn collect_facts(source: &str, root: Node<'_>) -> Result<SyntaxFacts, ParseFailure> {
    let mut facts = SyntaxFacts::default();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        match node.kind() {
            "function_definition" => {
                if let Some(fact) = function_definition_fact(source, node)? {
                    collect_function_body_facts(source, node, &fact, &mut facts)?;
                    facts.functions.push(fact);
                }
            }
            "declaration" | "type_definition"
                if !has_ancestor_kind(node, "function_definition") =>
            {
                if let Some(fact) = prototype_fact(source, node)? {
                    facts.functions.push(fact);
                }
            }
            "declaration" => {
                collect_declaration_facts(source, node, &mut facts)?;
                if let Some(fact) = shared_declaration_fact(source, node)? {
                    facts.shared_declarations.push(fact);
                }
            }
            "enumerator" => {
                if let Some(fact) = enum_fact(source, node)? {
                    facts.enum_constants.push(fact);
                }
            }
            "array_declarator" => facts.arrays.push(array_fact(source, node)?),
            "if_statement" => {
                collect_control_body_fact(node.child_by_field_name("consequence"), &mut facts)?;
                // An alternative that is itself an `if` is `else if`, which is
                // one construct written on one line. Treating it as a body to
                // put on its own line splits it into an `else` holding a
                // nested `if` — a line longer and a level deeper, in a Norm
                // that counts both.
                collect_control_body_fact(
                    node.child_by_field_name("alternative")
                        .and_then(alternative_statement)
                        .filter(|body| body.kind() != "if_statement"),
                    &mut facts,
                )?;
                if let Some(fact) = redundant_else_fact(source, node)? {
                    facts.redundant_else_branches.push(fact);
                }
            }
            // A bare `;` parses as an expression statement with no expression:
            // valid C that executes nothing.
            //
            // Two conditions carry the proof that deleting it changes nothing.
            // First, the parent must be a block or the file itself: the same
            // node shape is also how `while (x);` and `for (;;);` spell an
            // empty body, and deleting one of those would silently promote the
            // next statement into the loop. Second, no preprocessor directive
            // may sit immediately before it, because then the `;` may
            // terminate a statement that exists in only one build
            // configuration, which this parse cannot see.
            // The grammar names the operator of a binary or assignment
            // expression, and gives unary `-a` and `a++` their own node kinds.
            // That distinction is the whole proof: only an operator with an
            // operand on each side takes a space on each side, and reading it
            // from the tree cannot confuse the two the way a text scan can.
            // A declarator star binds to the name, and the grammar tells it
            // apart from multiplication by node kind rather than by guessing
            // from the surrounding spaces.
            "pointer_declarator" | "abstract_pointer_declarator" => {
                collect_pointer_star(node, &mut facts)?;
            }
            "compound_statement" => {
                facts
                    .compound_bodies
                    .push(node_range(node.start_byte(), node.end_byte())?);
                collect_crowded_statements(node, &mut facts)?;
            }
            "expression_statement" if is_stray_semicolon(node) => {
                facts
                    .empty_statements
                    .push(node_range(node.start_byte(), node.end_byte())?);
            }
            "expression_statement" | "return_statement" => {
                if let Some(fact) = ternary_fact(source, node)? {
                    facts.ternary_statements.push(fact);
                }
                if let Some(fact) = chained_assignment_fact(source, node)? {
                    facts.chained_assignments.push(fact);
                }
            }
            "while_statement" | "for_statement" | "do_statement" => {
                collect_control_body_fact(node.child_by_field_name("body"), &mut facts)?;
                facts.loops.push(loop_fact(source, node)?);
                if node.kind() == "for_statement" {
                    if let Some(fact) = for_loop_fact(node)? {
                        facts.for_loops.push(fact);
                    }
                }
            }
            "binary_expression" | "assignment_expression" => {
                collect_binary_operator(node, &mut facts)?;
                if let Some(fact) = null_check_fact(source, node)? {
                    facts.null_checks.push(fact);
                }
            }
            "struct_specifier" | "union_specifier" | "enum_specifier" => {
                if let Some(fact) = type_tag_fact(source, node)? {
                    facts.type_tags.push(fact);
                }
            }
            "call_expression" => {
                if let Some(fact) = call_fact(source, node)? {
                    facts.calls.push(fact);
                }
            }
            kind if kind.starts_with("preproc_") => {
                facts
                    .preprocessor_ranges
                    .push(node_range(node.start_byte(), node.end_byte())?);
                if null_provider(source, node)? {
                    facts
                        .null_providers
                        .push(node_range(node.start_byte(), node.end_byte())?);
                } else if null_invalidator(source, node)? {
                    facts
                        .null_invalidators
                        .push(node_range(node.start_byte(), node.end_byte())?);
                }
                if let Some(fact) = macro_fact(source, node)? {
                    facts.macros.push(fact);
                }
            }
            _ => {}
        }
        let mut cursor = node.walk();
        let children = node.children(&mut cursor).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    // A candidate only survives if its name appears exactly once in the whole
    // file: once in the tree, so nothing reads it, and once in the raw text, so
    // no macro body mentions it where the tree cannot see.
    if !facts.inert_declarations.is_empty() {
        let words = word_counts(source);
        facts
            .inert_declarations
            .retain(|candidate| words.get(candidate.name.as_str()).copied() == Some(1));
    }
    sort_facts(&mut facts);
    Ok(facts)
}

fn sort_facts(facts: &mut SyntaxFacts) {
    facts.functions.sort_by_key(|fact| fact.range);
    facts.enum_constants.sort_by_key(|fact| fact.name_range);
    facts.arrays.sort_by_key(|fact| fact.range);
    facts.preprocessor_ranges.sort();
    facts.preprocessor_ranges.dedup();
    facts
        .single_statement_bodies
        .sort_by_key(|fact| fact.compound_range);
    facts.control_compounds.sort();
    facts.control_compounds.dedup();
    facts
        .redundant_else_branches
        .sort_by_key(|fact| fact.else_keyword_range);
    facts.local_declarations.sort_by_key(|fact| fact.range);
    facts
        .initial_declaration_blocks
        .sort_by_key(|fact| fact.declarations.first().copied());
    facts.returns.sort_by_key(|fact| fact.range);
    facts.null_checks.sort_by_key(|fact| fact.range);
    facts.null_providers.sort();
    facts.null_providers.dedup();
    facts.null_invalidators.sort();
    facts.null_invalidators.dedup();
    facts.type_tags.sort_by_key(|fact| fact.name_range);
    facts.type_tags.dedup();
    facts.calls.sort_by_key(|fact| fact.name_range);
    facts.macros.sort_by_key(|fact| fact.name_range);
    facts.loops.sort_by_key(|fact| fact.range);
}
