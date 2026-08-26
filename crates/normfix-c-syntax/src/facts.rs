//! Backend-neutral structural facts extracted during the single C parse.

mod bodies;
mod collector;
mod conditionals;
mod declarations;
mod functions;
mod layout;
mod loops;
mod model;
mod nodes;
mod symbols;

pub(crate) use collector::collect_facts;
pub use model::{
    ArrayDeclaratorFact, CFunctionFact, CFunctionKind, CParameterFact, CStatementKind,
    CTypeTagKind, CallFact, ChainedAssignmentFact, EnumConstantFact, ForLoopFact,
    InitialDeclarationBlockFact, LocalDeclarationFact, LoopFact, MacroFact, NullCheckFact,
    RedundantElseFact, ReturnFact, SharedDeclarationFact, SingleStatementBodyFact, SyntaxFacts,
    TernaryFact, TernaryForm, TypeTagFact,
};
