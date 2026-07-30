//! Explicit authorization tokens for destructive planners.

use std::collections::BTreeSet;

use thiserror::Error;

/// The exact acknowledgement accepted by interactive authorization.
///
/// The CLI may localize the surrounding prompt, but the destructive grant is
/// minted only after this exact, deliberately conspicuous phrase is entered.
pub const EXACT_CONFIRMATION_PHRASE: &str = "I UNDERSTAND THIS MAY DELETE OR MOVE FILES";

/// One destructive capability that can be granted independently.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DestructiveCapability {
    /// Remove `static` function definitions proven unreachable in a closed set.
    RemoveUnreferencedStaticFunctions,
    /// Move unexpected regular files to an external recovery directory.
    QuarantineUnexpectedFiles,
}

/// The origin of an explicit destructive grant.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AuthorizationMethod {
    /// A person answered yes to the explicit terminal warning.
    InteractiveYes,
    /// A person typed the exact acknowledgement in an interactive terminal.
    ExactInteractiveConfirmation,
    /// Both the unsafe-mode and force switches were explicitly supplied.
    UnsafeAndForceFlags,
}

/// A non-empty set of destructive capabilities requested by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructiveRequest {
    capabilities: BTreeSet<DestructiveCapability>,
}

impl DestructiveRequest {
    /// Builds a request for one capability.
    #[must_use]
    pub fn one(capability: DestructiveCapability) -> Self {
        Self {
            capabilities: BTreeSet::from([capability]),
        }
    }

    /// Builds a request from several capabilities.
    ///
    /// # Errors
    ///
    /// Returns [`AuthorizationError::EmptyRequest`] for an empty iterator.
    pub fn new(
        capabilities: impl IntoIterator<Item = DestructiveCapability>,
    ) -> Result<Self, AuthorizationError> {
        let capabilities = capabilities.into_iter().collect::<BTreeSet<_>>();
        if capabilities.is_empty() {
            return Err(AuthorizationError::EmptyRequest);
        }
        Ok(Self { capabilities })
    }

    /// Returns the requested capabilities in stable order.
    #[must_use]
    pub fn capabilities(&self) -> impl ExactSizeIterator<Item = DestructiveCapability> + '_ {
        self.capabilities.iter().copied()
    }

    /// Mints a grant after an exact interactive acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns [`AuthorizationError::ConfirmationMismatch`] unless `entered`
    /// exactly equals [`EXACT_CONFIRMATION_PHRASE`].
    pub fn authorize_interactively(
        &self,
        entered: &str,
    ) -> Result<DestructiveAuthorization, AuthorizationError> {
        if entered != EXACT_CONFIRMATION_PHRASE {
            return Err(AuthorizationError::ConfirmationMismatch);
        }
        Ok(DestructiveAuthorization {
            capabilities: self.capabilities.clone(),
            method: AuthorizationMethod::ExactInteractiveConfirmation,
        })
    }

    /// Mints a grant after an explicit interactive `y/N` confirmation.
    ///
    /// # Errors
    ///
    /// Returns [`AuthorizationError::ConfirmationDeclined`] when the answer
    /// was not affirmative. This exists for the documented CLI workflow; API
    /// clients that need a stronger acknowledgement can use the exact phrase.
    pub fn authorize_yes(
        &self,
        confirmed: bool,
    ) -> Result<DestructiveAuthorization, AuthorizationError> {
        if !confirmed {
            return Err(AuthorizationError::ConfirmationDeclined);
        }
        Ok(DestructiveAuthorization {
            capabilities: self.capabilities.clone(),
            method: AuthorizationMethod::InteractiveYes,
        })
    }

    /// Mints a non-interactive grant only when both safety switches are set.
    ///
    /// # Errors
    ///
    /// Returns [`AuthorizationError::UnsafeFlagRequired`] or
    /// [`AuthorizationError::ForceFlagRequired`] when either explicit switch
    /// is missing.
    pub fn authorize_forced(
        &self,
        unsafe_enabled: bool,
        force_enabled: bool,
    ) -> Result<DestructiveAuthorization, AuthorizationError> {
        if !unsafe_enabled {
            return Err(AuthorizationError::UnsafeFlagRequired);
        }
        if !force_enabled {
            return Err(AuthorizationError::ForceFlagRequired);
        }
        Ok(DestructiveAuthorization {
            capabilities: self.capabilities.clone(),
            method: AuthorizationMethod::UnsafeAndForceFlags,
        })
    }
}

/// An unforgeable-by-fields grant accepted by destructive planners.
///
/// Its fields are private; callers obtain it only through a validated
/// [`DestructiveRequest`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructiveAuthorization {
    capabilities: BTreeSet<DestructiveCapability>,
    method: AuthorizationMethod,
}

impl DestructiveAuthorization {
    /// Returns how this grant was explicitly authorized.
    #[must_use]
    pub const fn method(&self) -> AuthorizationMethod {
        self.method
    }

    /// Returns whether this grant contains `capability`.
    #[must_use]
    pub fn allows(&self, capability: DestructiveCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    pub(crate) fn require(
        &self,
        capability: DestructiveCapability,
    ) -> Result<(), AuthorizationError> {
        if self.allows(capability) {
            Ok(())
        } else {
            Err(AuthorizationError::CapabilityNotGranted(capability))
        }
    }
}

/// A destructive request could not be authorized or used.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorizationError {
    /// A capability request contained no operation.
    #[error("at least one destructive capability must be requested")]
    EmptyRequest,
    /// Interactive input did not exactly match the acknowledgement phrase.
    #[error("the destructive confirmation phrase did not match exactly")]
    ConfirmationMismatch,
    /// The interactive y/N prompt was declined.
    #[error("destructive operations were not confirmed")]
    ConfirmationDeclined,
    /// Non-interactive authorization omitted unsafe mode.
    #[error("non-interactive destructive authorization requires unsafe mode")]
    UnsafeFlagRequired,
    /// Non-interactive authorization omitted the force switch.
    #[error("non-interactive destructive authorization requires the force switch")]
    ForceFlagRequired,
    /// A grant was valid but did not cover the requested planner.
    #[error("destructive capability was not granted: {0:?}")]
    CapabilityNotGranted(DestructiveCapability),
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorizationError, AuthorizationMethod, DestructiveCapability, DestructiveRequest,
        EXACT_CONFIRMATION_PHRASE,
    };

    #[test]
    fn exact_phrase_and_both_flags_are_required() {
        let request =
            DestructiveRequest::one(DestructiveCapability::RemoveUnreferencedStaticFunctions);
        assert_eq!(
            request.authorize_interactively("yes"),
            Err(AuthorizationError::ConfirmationMismatch)
        );
        assert_eq!(
            request.authorize_yes(false),
            Err(AuthorizationError::ConfirmationDeclined)
        );
        assert_eq!(
            request
                .authorize_yes(true)
                .expect("affirmative y/N confirmation")
                .method(),
            AuthorizationMethod::InteractiveYes
        );
        let interactive = request
            .authorize_interactively(EXACT_CONFIRMATION_PHRASE)
            .expect("exact confirmation");
        assert_eq!(
            interactive.method(),
            AuthorizationMethod::ExactInteractiveConfirmation
        );

        assert_eq!(
            request.authorize_forced(false, true),
            Err(AuthorizationError::UnsafeFlagRequired)
        );
        assert_eq!(
            request.authorize_forced(true, false),
            Err(AuthorizationError::ForceFlagRequired)
        );
        request
            .authorize_forced(true, true)
            .expect("both explicit flags");
    }

    #[test]
    fn grants_are_capability_scoped() {
        let request = DestructiveRequest::one(DestructiveCapability::QuarantineUnexpectedFiles);
        let grant = request
            .authorize_forced(true, true)
            .expect("authorized quarantine");
        assert!(grant.allows(DestructiveCapability::QuarantineUnexpectedFiles));
        assert!(!grant.allows(DestructiveCapability::RemoveUnreferencedStaticFunctions));
    }
}
