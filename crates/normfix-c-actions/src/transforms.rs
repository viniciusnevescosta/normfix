//! Ordered native C transformation phases.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use normfix_c_syntax::{CFunctionKind, TernaryForm};
use normfix_core::TextRange;
use regex::Regex;

use crate::analysis::{
    IncludeOrderKey, function_infos, include_order_key, is_identifier, matching_forward,
};
use crate::context::{ParsedContext, Token};
use crate::edit::Edit;
use crate::source::{
    LexicalMap, PhysicalLine, SourceLines, escaped_physical_newline, leading_whitespace,
    visual_width, visual_width_from, whitespace_after, whitespace_before,
};
use crate::{Applicability, CActionError, CActionOptions, ReportedDiagnostic};

mod blocks;
mod continuations;
mod declarations;
mod functions;
mod indentation;
mod preprocessor;
mod semantics;
mod spacing;
mod statements;
mod wrapping;

use blocks::{fix_blank_lines, fix_braces_and_controls};
use continuations::compact_continuations;
use declarations::{
    align_declarations, decimal_mantissa_regex, format_initial_declarations, hex_mantissa_regex,
    is_declaration_word,
};
use functions::{fix_function_layout, next_tab_stop, tabs_to_column};
use indentation::{fix_indentation, indentation_model, is_control_header, whitespace_run_near};
use preprocessor::{
    format_preprocessors, has_sensitive_line_end, preprocessor_line_set, remove_invalid_comments,
    reorder_includes,
};
use semantics::{
    add_void_to_definitions, compact_null_checks, parenthesize_returns,
    replace_pointer_zero_returns,
};
use spacing::{fix_token_spacing, inside_numeric_exponent, multiline_preprocessor_lines};
use statements::{
    control_condition_close, find_brace_near, remove_empty_statements, remove_redundant_else,
    remove_single_statement_braces, remove_unused_variables, rewrite_for_loops, rewrite_ternaries,
    separate_crowded_statements, split_chained_assignments, split_declarations,
    split_shared_declarations,
};
use wrapping::wrap_long_lines;

#[derive(Clone, Debug)]
pub(crate) struct ActionBatch {
    pub(crate) rule_id: &'static str,
    pub(crate) applicability: Applicability,
    pub(crate) edits: Vec<Edit>,
}

impl ActionBatch {
    fn layout(rule_id: &'static str, edits: Vec<Edit>) -> Option<Self> {
        (!edits.is_empty()).then_some(Self {
            rule_id,
            applicability: Applicability::SafeLayout,
            edits,
        })
    }

    fn semantic(rule_id: &'static str, edits: Vec<Edit>) -> Option<Self> {
        (!edits.is_empty()).then_some(Self {
            rule_id,
            applicability: Applicability::SafeSemantic,
            edits,
        })
    }

    fn destructive(rule_id: &'static str, edits: Vec<Edit>) -> Option<Self> {
        (!edits.is_empty()).then_some(Self {
            rule_id,
            applicability: Applicability::UnsafeDestructive,
            edits,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Phase {
    Preprocessor,
    IncludeOrder,
    InvalidComments,
    UnusedVariables,
    EmptyStatements,
    CompactContinuations,
    BlankLines,
    BracesAndControls,
    CrowdedStatements,
    SingleStatementBlocks,
    RedundantElse,
    ForLoops,
    Ternaries,
    ChainedAssignments,
    SharedDeclarations,
    SplitDeclarations,
    FunctionLayout,
    Indentation,
    InitialDeclarations,
    TokenSpacing,
    Declarations,
    PointerNullReturns,
    CompactNullChecks,
    ReturnParentheses,
    DefinitionVoid,
    LongLines,
}

pub(crate) fn phases(options: &CActionOptions) -> Vec<Phase> {
    let mut result = vec![Phase::Preprocessor];
    if options.reorder_includes {
        result.push(Phase::IncludeOrder);
    }
    if options.remove_invalid_comments {
        result.push(Phase::InvalidComments);
    }
    if options.remove_unused_variables {
        result.push(Phase::UnusedVariables);
    }
    result.extend([
        Phase::EmptyStatements,
        Phase::CompactContinuations,
        Phase::BlankLines,
        Phase::BracesAndControls,
        Phase::CrowdedStatements,
        Phase::SingleStatementBlocks,
        Phase::RedundantElse,
        Phase::ForLoops,
        Phase::Ternaries,
        Phase::ChainedAssignments,
        Phase::SharedDeclarations,
        Phase::SplitDeclarations,
        Phase::FunctionLayout,
        Phase::Indentation,
        Phase::InitialDeclarations,
        Phase::TokenSpacing,
        Phase::Declarations,
        Phase::PointerNullReturns,
        Phase::CompactNullChecks,
        Phase::ReturnParentheses,
        Phase::DefinitionVoid,
        Phase::LongLines,
    ]);
    result
}

impl Phase {
    pub(crate) const fn one_shot(self) -> bool {
        // A loop nested in another is only reachable once the outer one has
        // become a `while`, so this phase has to be allowed to come back.
        !matches!(
            self,
            Self::CompactContinuations
                | Self::LongLines
                | Self::ForLoops
                | Self::ChainedAssignments
        )
    }

    // One arm per phase: the dispatch is a table, and splitting it would hide
    // the order the phases run in, which is what makes them compose.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn plan(
        self,
        context: &ParsedContext,
        diagnostics: &[ReportedDiagnostic],
        options: &CActionOptions,
    ) -> Result<Option<ActionBatch>, CActionError> {
        match self {
            Self::Preprocessor => Ok(ActionBatch::layout(
                "PREPROCESSOR_SPACING",
                format_preprocessors(context)?,
            )),
            Self::IncludeOrder => Ok(ActionBatch::semantic(
                "INCLUDE_ORDER",
                reorder_includes(context)?,
            )),
            Self::InvalidComments => Ok(ActionBatch::destructive(
                "REMOVE_INVALID_COMMENT",
                remove_invalid_comments(context, diagnostics)?,
            )),
            Self::EmptyStatements => Ok(ActionBatch::semantic(
                "REMOVE_EMPTY_STATEMENT",
                remove_empty_statements(context)?,
            )),
            // Proven by the fact, permitted by the option: the applicability
            // says how it is known to be safe, and `--unsafe` says whether the
            // reader asked for it at all.
            Self::UnusedVariables => Ok(ActionBatch::semantic(
                "REMOVE_UNUSED_VARIABLE",
                remove_unused_variables(context)?,
            )),
            Self::CompactContinuations => Ok(ActionBatch::layout(
                "COMPACT_CONTINUATION",
                compact_continuations(context, options.max_columns)?,
            )),
            Self::BlankLines => Ok(ActionBatch::layout(
                "BLANK_LINE_LAYOUT",
                fix_blank_lines(context, diagnostics)?,
            )),
            Self::BracesAndControls => Ok(ActionBatch::layout(
                "BRACE_CONTROL_LAYOUT",
                fix_braces_and_controls(context, diagnostics)?,
            )),
            Self::CrowdedStatements => Ok(ActionBatch::layout(
                "ONE_INSTRUCTION_PER_LINE",
                separate_crowded_statements(context)?,
            )),
            Self::SingleStatementBlocks => Ok(ActionBatch::semantic(
                "REMOVE_SINGLE_STATEMENT_BRACES",
                remove_single_statement_braces(context)?,
            )),
            Self::ForLoops => Ok(ActionBatch::semantic(
                "REPLACE_FOR_LOOP",
                rewrite_for_loops(context)?,
            )),
            Self::Ternaries => Ok(ActionBatch::semantic(
                "REPLACE_TERNARY",
                rewrite_ternaries(context)?,
            )),
            Self::ChainedAssignments => Ok(ActionBatch::semantic(
                "SPLIT_CHAINED_ASSIGNMENT",
                split_chained_assignments(context)?,
            )),
            Self::SharedDeclarations => Ok(ActionBatch::semantic(
                "ONE_DECLARATION_PER_LINE",
                split_shared_declarations(context)?,
            )),
            Self::SplitDeclarations => Ok(ActionBatch::semantic(
                "SPLIT_DECLARATION_ASSIGNMENT",
                split_declarations(context)?,
            )),
            Self::RedundantElse => Ok(ActionBatch::semantic(
                "REMOVE_REDUNDANT_ELSE",
                remove_redundant_else(context)?,
            )),
            Self::FunctionLayout => Ok(ActionBatch::layout(
                "FUNCTION_LAYOUT",
                fix_function_layout(context, diagnostics, options)?,
            )),
            Self::Indentation => Ok(ActionBatch::layout(
                "INDENTATION",
                fix_indentation(context, diagnostics)?,
            )),
            Self::InitialDeclarations => Ok(ActionBatch::layout(
                "INITIAL_DECLARATION_LAYOUT",
                format_initial_declarations(context)?,
            )),
            Self::TokenSpacing => Ok(ActionBatch::layout(
                "TOKEN_SPACING",
                fix_token_spacing(context, diagnostics)?,
            )),
            Self::Declarations => Ok(ActionBatch::layout(
                "DECLARATION_ALIGNMENT",
                align_declarations(context, diagnostics, options)?,
            )),
            Self::PointerNullReturns => Ok(ActionBatch::semantic(
                "POINTER_NULL_RETURN",
                replace_pointer_zero_returns(context)?,
            )),
            Self::CompactNullChecks => {
                if options.compact_null_checks {
                    Ok(ActionBatch::semantic(
                        "COMPACT_NULL_CHECK",
                        compact_null_checks(context)?,
                    ))
                } else {
                    Ok(None)
                }
            }
            Self::ReturnParentheses => Ok(ActionBatch::semantic(
                "RETURN_PARENTHESIS",
                parenthesize_returns(context, diagnostics)?,
            )),
            Self::DefinitionVoid => Ok(ActionBatch::semantic(
                "NO_ARGS_VOID",
                add_void_to_definitions(context, diagnostics)?,
            )),
            Self::LongLines => Ok(ActionBatch::layout(
                "LINE_TOO_LONG",
                wrap_long_lines(context, options.max_columns)?,
            )),
        }
    }
}
