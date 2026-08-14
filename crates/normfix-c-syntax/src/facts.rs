//! Backend-neutral structural facts extracted during the single C parse.

use normfix_core::{TextRange, TextSize};
use std::collections::HashMap;

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
    /// Operators that take an operand on each side, so each side takes a space.
    pub binary_operators: Vec<TextRange>,
    /// The `*` of a pointer declarator, which binds to the name after it.
    pub pointer_stars: Vec<TextRange>,
    /// Control-statement bodies that are a single statement, not a block.
    pub control_inline_bodies: Vec<TextRange>,
    /// Declarations whose initializer can become a separate assignment.
    pub declaration_splits: Vec<DeclarationSplitFact>,
    /// Local declarations nothing reads, whose removal deletes nothing that
    /// runs.
    pub inert_declarations: Vec<UnusedLocalFact>,
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
    /// Statements whose entire value is one forbidden conditional expression.
    pub ternary_statements: Vec<TernaryFact>,
}

/// A statement that exists only to choose between two values.
///
/// The Norm forbids `?:` outright, and the rewrite that removes it has to know
/// more than where the operator is: what the statement does with the value it
/// picks, and how much room the enclosing function has left, since one line
/// becomes three or four.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TernaryFact {
    /// The whole statement, replaced as a unit.
    pub statement_range: TextRange,
    /// What the statement does with the chosen value.
    pub form: TernaryForm,
    /// The condition being tested.
    pub condition_range: TextRange,
    /// The value taken when the condition holds.
    pub consequence_range: TextRange,
    /// The value taken otherwise.
    pub alternative_range: TextRange,
    /// Whether the condition already carries its own parentheses.
    pub condition_parenthesized: bool,
    /// Body of the function that owns the statement, whose line budget the
    /// rewrite spends.
    pub function_body_range: TextRange,
}

/// What a ternary statement does with the value it selects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TernaryForm {
    /// `target = cond ? a : b;`, keeping whichever assignment operator was
    /// written.
    Assignment {
        /// The text naming what is assigned to.
        target: String,
        /// The assignment operator, which may be compound.
        operator: String,
    },
    /// `return (cond ? a : b);`
    Return,
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

/// A local nothing reads, holding nothing that runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnusedLocalFact {
    /// The whole declaration.
    pub range: TextRange,
    /// The name it declares.
    pub name: String,
}

/// A declaration that can be split into a declaration and an assignment.
///
/// The official checker calls the single-line form `DECL_ASSIGN_LINE`. Four
/// shapes are never recorded here, because for them the split would be a
/// different program: `const`, which cannot be assigned after its declaration;
/// an aggregate initializer such as `{1, 2}`, which is initialization syntax
/// and not an expression; `static`, which is initialized once where an
/// assignment would run on every call; and a declaration naming more than one
/// variable, whose initializers cannot be told apart from one range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationSplitFact {
    /// The whole declaration, so a caller can match it to its block.
    pub declaration_range: TextRange,
    /// The ` = value` to remove, leaving the declaration behind.
    pub strip_range: TextRange,
    /// The identifier the assignment names, without any pointer stars.
    pub name: String,
    /// The initializer expression the assignment takes.
    pub value_range: TextRange,
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
            "declaration" => collect_declaration_facts(source, node, &mut facts)?,
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
            }
            "while_statement" | "for_statement" | "do_statement" => {
                collect_control_body_fact(node.child_by_field_name("body"), &mut facts)?;
                facts.loops.push(loop_fact(source, node)?);
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

/// Records the `*` that binds a declarator to its name.
///
/// The grammar tells this star apart from multiplication by node kind rather
/// than by guessing from the surrounding spaces.
fn collect_pointer_star(node: Node<'_>, facts: &mut SyntaxFacts) -> Result<(), ParseFailure> {
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
fn collect_binary_operator(node: Node<'_>, facts: &mut SyntaxFacts) -> Result<(), ParseFailure> {
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
fn is_stray_semicolon(node: Node<'_>) -> bool {
    direct_named_children(node).next().is_none()
        && node.parent().is_some_and(|parent| {
            matches!(parent.kind(), "compound_statement" | "translation_unit")
        })
        && !node
            .prev_sibling()
            .is_some_and(|sibling| sibling.kind().starts_with("preproc"))
}

/// Records what a local declaration allows: splitting it, or deleting it.
fn collect_declaration_facts(
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
fn word_counts(source: &str) -> HashMap<&str, usize> {
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

/// The identifier a declarator finally names, past any pointer stars.
fn innermost_identifier(node: Node<'_>) -> Option<Node<'_>> {
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

fn collect_control_body_fact(
    body: Option<Node<'_>>,
    facts: &mut SyntaxFacts,
) -> Result<(), ParseFailure> {
    let Some(body) = body else {
        return Ok(());
    };
    if body.kind() != "compound_statement" {
        // A single-statement body still owns its own line. Recording it here
        // means the rule reads the tree instead of guessing where a condition
        // ended by scanning parentheses in the text.
        facts
            .control_inline_bodies
            .push(node_range(body.start_byte(), body.end_byte())?);
        return Ok(());
    }
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
fn ternary_fact(source: &str, node: Node<'_>) -> Result<Option<TernaryFact>, ParseFailure> {
    if node.parent().is_none_or(|parent| parent.kind() != "compound_statement")
        || contains_kind(node, "comment")
    {
        return Ok(None);
    }
    let Some(value) = node.named_child(0) else {
        return Ok(None);
    };
    let (form, conditional) = if node.kind() == "return_statement" {
        (TernaryForm::Return, unwrap_parentheses(value))
    } else {
        let (Some(target), Some(operator), Some(right)) = (
            value.child_by_field_name("left"),
            value.child_by_field_name("operator"),
            value.child_by_field_name("right"),
        ) else {
            return Ok(None);
        };
        if value.kind() != "assignment_expression"
            || ["call_expression", "update_expression", "assignment_expression"]
                .iter()
                .any(|kind| contains_kind(target, kind))
        {
            return Ok(None);
        }
        let form = TernaryForm::Assignment {
            target: node_text(source, target)?.to_owned(),
            operator: node_text(source, operator)?.to_owned(),
        };
        (form, unwrap_parentheses(right))
    };
    let (Some(condition), Some(consequence), Some(alternative)) = (
        conditional.child_by_field_name("condition"),
        conditional.child_by_field_name("consequence"),
        conditional.child_by_field_name("alternative"),
    ) else {
        return Ok(None);
    };
    if conditional.kind() != "conditional_expression"
        || [condition, consequence, alternative]
            .iter()
            .any(|part| contains_kind(*part, "conditional_expression"))
    {
        return Ok(None);
    }
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

/// Peels the parentheses a value was written inside, which change no meaning.
fn unwrap_parentheses(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == "parenthesized_expression" {
        let Some(inner) = node.named_child(0) else {
            return node;
        };
        node = inner;
    }
    node
}

/// Whether `kind` appears anywhere at or below `node`.
fn contains_kind(node: Node<'_>, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|child| contains_kind(child, kind))
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
