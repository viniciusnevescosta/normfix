//! Read-only planners for explicitly destructive `norminette-fix` operations.
//!
//! This crate never changes the filesystem. It can propose removal of a
//! conservatively proven dead `static` function or capture a recoverable
//! quarantine plan for an unexpected regular file. Callers must still execute
//! accepted plans through the transactional writer and revalidate every hash
//! immediately before committing.

#![forbid(unsafe_code)]

mod authorization;
mod quarantine;
mod static_functions;

pub use authorization::{
    AuthorizationError, AuthorizationMethod, DestructiveAuthorization, DestructiveCapability,
    DestructiveRequest, EXACT_CONFIRMATION_PHRASE,
};
pub use quarantine::{
    QuarantineItem, QuarantinePlan, QuarantinePlanError, QuarantineRequest, QuarantineSnapshot,
    plan_quarantine,
};
pub use static_functions::{
    ClosedCSourceSet, ClosedSourceError, DestructiveFilePlan, StaticRemovalPlan,
    StaticRemovalPlanError, plan_unused_static_functions,
};
