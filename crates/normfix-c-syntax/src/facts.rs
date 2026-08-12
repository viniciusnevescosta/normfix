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
    /// Control-statement bodies that are compound blocks with one proven
    /// scope-free statement.
    pub single_statement_bodies: Vec<SingleStatementBodyFact>,
    /// Every compound body directly owned by a control statement.
    pub control_compounds: Vec<TextRange>,
    /// Every compound statement, including function bodies and bare blocks.
    pub compound_bodies: Vec<TextRange>,
    /// `if`/`else` shapes whose alternative can follow a terminal return.
    pub redundant_else_branches: Vec<RedundantElseFact>,
    /// Statements that are a lone `;`, which do nothing at all.
    pub empty_statements: Vec<TextRange>,
    /// Direct local declarations in function bodies.
    pub local_declarations: Vec<LocalDeclarationFact>,
    /// Initial declaration blocks and the first following instruction.
    pub initial_declaration_blocks: Vec<InitialDeclarationBlockFact>,
    /// Return statements in function definitions.
    pub returns: Vec<ReturnFact>,
    /// Simple `identifier == NULL` and `identifier != NULL` expressions.
    pub null_checks: Vec<NullCheckFact>,
    /// Source locations that make the standard `NULL` macro available.
    pub null_providers: Vec<TextRange>,
    /// Directives that undefine or replace `NULL` with an unproven value.
    pub null_invalidators: Vec<TextRange>,
    /// Named structure, union, and enumeration tags.
    pub type_tags: Vec<TypeTagFact>,
    /// Calls whose callee is one simple identifier.
    pub calls: Vec<CallFact>,
    /// Object-like and function-like macros declared in this source.
    pub macros: Vec<MacroFact>,
    /// Syntactically obvious unbounded loops and their exit evidence.
    pub loops: Vec<LoopFact>,
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
    /// Recoverable named parameters, including function-pointer declarators.
    pub parameters: Vec<CParameterFact>,
    /// Whether `static` occurs among definition/declaration specifiers.
    pub is_static: bool,
    /// Whether the declarator proves that the function returns a pointer.
    /// Typedef-hidden pointers deliberately remain `false`.
    pub returns_pointer: bool,
    /// Definition or prototype.
    pub kind: CFunctionKind,
}

/// One recoverable named function parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CParameterFact {
    /// Declared parameter identifier.
    pub name: String,
    /// Identifier byte range.
    pub name_range: TextRange,
}

/// A conservative statement classification used by source actions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CStatementKind {
    /// An expression followed by a semicolon.
    Expression,
    /// A return statement.
    Return,
    /// A break statement.
    Break,
    /// A continue statement.
    Continue,
}

/// A compound control body containing exactly one scope-free statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleStatementBodyFact {
    /// Complete compound statement, including braces.
    pub compound_range: TextRange,
    /// The sole statement without leading trivia.
    pub statement_range: TextRange,
    /// Narrow statement classification.
    pub statement_kind: CStatementKind,
}

/// An `else` branch that may safely follow a returning `if` consequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedundantElseFact {
    /// The `else` keyword alone.
    pub else_keyword_range: TextRange,
    /// Complete alternative statement or compound block.
    pub alternative_range: TextRange,
    /// The one return statement to keep after removing `else` and any braces.
    pub return_range: TextRange,
}

/// One direct local declaration in a function's outer compound body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalDeclarationFact {
    /// Function that owns the declaration.
    pub function_name: String,
    /// Complete declaration.
    pub range: TextRange,
    /// First declared identifier, when unambiguous.
    pub name: Option<String>,
    /// First identifier range, when unambiguous.
    pub name_range: Option<TextRange>,
    /// Compound scope that owns this declaration.
    pub scope_range: TextRange,
    /// Whether the declaration belongs to the initial declaration block.
    pub initial: bool,
}

/// A function's non-empty initial declaration block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialDeclarationBlockFact {
    /// Function that owns the block.
    pub function_name: String,
    /// Declarations in source order.
    pub declarations: Vec<TextRange>,
    /// First instruction after the block, if present.
    pub following_item: Option<TextRange>,
}

/// One return expression and its enclosing function's proven return shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnFact {
    /// Enclosing function name.
    pub function_name: String,
    /// Complete return statement.
    pub range: TextRange,
    /// Return expression, excluding the `return` keyword and semicolon.
    pub expression_range: Option<TextRange>,
    /// Whether the enclosing declarator explicitly returns a pointer.
    pub function_returns_pointer: bool,
}

/// Equality or inequality against the `NULL` identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NullCheckFact {
    /// Complete binary expression.
    pub range: TextRange,
    /// Simple non-`NULL` identifier.
    pub operand: String,
    /// `true` for `==`, `false` for `!=`.
    pub equals: bool,
}

/// Named C tag category.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CTypeTagKind {
    /// `struct` tag.
    Struct,
    /// `union` tag.
    Union,
    /// `enum` tag.
    Enum,
}

/// One explicitly named C structure, union, or enum tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeTagFact {
    /// Tag category.
    pub kind: CTypeTagKind,
    /// Identifier text.
    pub name: String,
    /// Identifier byte range.
    pub name_range: TextRange,
}

/// A call whose callee is a simple identifier rather than a member, pointer,
/// parenthesized expression, or macro-produced construct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallFact {
    /// Callee identifier.
    pub name: String,
    /// Identifier byte range.
    pub name_range: TextRange,
}

/// One macro definition whose name can shadow call-like source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroFact {
    /// Macro identifier.
    pub name: String,
    /// Identifier byte range.
    pub name_range: TextRange,
    /// Whether the definition has a macro parameter list.
    pub function_like: bool,
}

/// A loop that is syntactically unconditional plus conservative exit evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopFact {
    /// Complete loop statement.
    pub range: TextRange,
    /// Whether the loop condition is absent or a literal true value.
    pub unconditional: bool,
    /// Whether a return, goto, or loop-owning break can leave it.
    pub has_obvious_exit: bool,
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
            "enumerator" => {
                if let Some(fact) = enum_fact(source, node)? {
                    facts.enum_constants.push(fact);
                }
            }
            "array_declarator" => facts.arrays.push(array_fact(source, node)?),
            "if_statement" => {
                collect_control_body_fact(node.child_by_field_name("consequence"), &mut facts)?;
                collect_control_body_fact(
                    node.child_by_field_name("alternative")
                        .and_then(alternative_statement),
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
            "compound_statement" => {
                facts
                    .compound_bodies
                    .push(node_range(node.start_byte(), node.end_byte())?);
            }
            "expression_statement"
                if direct_named_children(node).next().is_none()
                    && node.parent().is_some_and(|parent| {
                        matches!(parent.kind(), "compound_statement" | "translation_unit")
                    })
                    && !node
                        .prev_sibling()
                        .is_some_and(|sibling| sibling.kind().starts_with("preproc")) =>
            {
                facts
                    .empty_statements
                    .push(node_range(node.start_byte(), node.end_byte())?);
            }
            "while_statement" | "for_statement" | "do_statement" => {
                collect_control_body_fact(node.child_by_field_name("body"), &mut facts)?;
                facts.loops.push(loop_fact(source, node)?);
            }
            "binary_expression" => {
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

fn prototype_fact(source: &str, node: Node<'_>) -> Result<Option<CFunctionFact>, ParseFailure> {
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

fn collect_control_body_fact(
    body: Option<Node<'_>>,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    let Some(body) = body.filter(|body| body.kind() == "compound_statement") else {
        return Ok(());
    };
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

fn redundant_else_fact(
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

fn alternative_statement(node: Node<'_>) -> Option<Node<'_>> {
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

fn collect_function_body_facts(
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

fn null_check_fact(source: &str, node: Node<'_>) -> Result<Option<NullCheckFact>, ParseFailure> {
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

fn null_provider(source: &str, node: Node<'_>) -> Result<bool, ParseFailure> {
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

fn null_invalidator(source: &str, node: Node<'_>) -> Result<bool, ParseFailure> {
    let text = node_text(source, node)?;
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(compact == "#undef NULL"
        || ((node.kind() == "preproc_def" || node.kind() == "preproc_function_def")
            && compact.starts_with("#define NULL ")))
}

fn type_tag_fact(source: &str, node: Node<'_>) -> Result<Option<TypeTagFact>, ParseFailure> {
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

fn call_fact(source: &str, node: Node<'_>) -> Result<Option<CallFact>, ParseFailure> {
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

fn macro_fact(source: &str, node: Node<'_>) -> Result<Option<MacroFact>, ParseFailure> {
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

fn loop_fact(source: &str, node: Node<'_>) -> Result<LoopFact, ParseFailure> {
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

fn single_declaration_name(declaration: Node<'_>) -> Option<Node<'_>> {
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
