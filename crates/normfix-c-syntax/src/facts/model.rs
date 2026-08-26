//! Public, backend-neutral fact model.

use normfix_core::TextRange;

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
    /// Forbidden `for` loops that a `while` can say exactly.
    pub for_loops: Vec<ForLoopFact>,
    /// Gaps where a second instruction begins on a line already holding one.
    pub crowded_statements: Vec<TextRange>,
    /// Declarations naming more than one variable at once.
    pub shared_declarations: Vec<SharedDeclarationFact>,
    /// Statements assigning one value to two names at once.
    pub chained_assignments: Vec<ChainedAssignmentFact>,
}

/// A statement that assigns through another assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainedAssignmentFact {
    /// The whole statement, replaced as a unit.
    pub statement_range: TextRange,
    /// What the outer assignment writes to.
    pub target: String,
    /// The outer operator, which may be compound.
    pub operator: String,
    /// The inner assignment exactly as written.
    pub inner: String,
    /// What the inner assignment writes to, and the outer then reads.
    pub inner_target: String,
    /// Body of the function that owns the statement, whose line budget the
    /// split spends.
    pub function_body_range: TextRange,
}

/// A declaration that names several variables, which the Norm splits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedDeclarationFact {
    /// The whole declaration, replaced as a unit.
    pub range: TextRange,
    /// The specifiers every declarator shares, such as `static int`.
    pub specifiers: String,
    /// Each declarator exactly as written, stars and all.
    pub declarators: Vec<String>,
}

/// A `for` loop the Norm forbids, and the three pieces a `while` needs.
///
/// The rewrite is only the same loop when the step still runs after every
/// iteration of the body, which is why the fact records enough to check that
/// nothing in the body jumps over it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForLoopFact {
    /// The whole loop, replaced as a unit.
    pub statement_range: TextRange,
    /// What runs once before the loop, if anything.
    pub initializer_range: Option<TextRange>,
    /// What is tested before each iteration; absent means it loops forever.
    pub condition_range: Option<TextRange>,
    /// What runs after each iteration, if anything.
    pub step_range: Option<TextRange>,
    /// The body, whether or not it is a block.
    pub body_range: TextRange,
    /// Whether the body already carries its own braces.
    pub body_is_block: bool,
    /// Body of the function that owns the loop, whose line budget it spends.
    pub function_body_range: TextRange,
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
