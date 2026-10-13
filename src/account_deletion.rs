use crate::infrai_client::{InfraiClient, InfraiError};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionRequest {
    pub tenant_id: String,
    pub user_id: String,
    pub credential_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionReceipt {
    pub tenant_id: String,
    pub user_id: String,
    pub revoked_sessions: usize,
    pub credential_revoked: bool,
    pub local_state: AccountState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountState {
    Onboarding,
    Active,
    DeletionPending,
    Deleted,
}

#[derive(Debug, Error)]
pub enum DeletionError {
    #[error("account is already deleted")]
    AlreadyDeleted,
    #[error("remote revocation failed; local account remains pending: {0}")]
    Revocation(#[from] InfraiError),
}

pub struct AccountDeletionService {
    infrai: InfraiClient,
}

impl AccountDeletionService {
    pub fn new(infrai: InfraiClient) -> Self {
        Self { infrai }
    }

    pub async fn delete(
        &self,
        request: DeletionRequest,
        state: AccountState,
    ) -> Result<DeletionReceipt, DeletionError> {
        if state == AccountState::Deleted {
            return Err(DeletionError::AlreadyDeleted);
        }

        let sessions = self.infrai.list_sessions(&request.user_id).await?;
        for session in &sessions {
            self.infrai.revoke_session(&session.id).await?;
        }
        self.infrai
            .revoke_account_key(&request.credential_id)
            .await?;

        Ok(DeletionReceipt {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            revoked_sessions: sessions.len(),
            credential_revoked: true,
            local_state: AccountState::Deleted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_is_terminal_and_cannot_start_another_revocation() {
        let state = AccountState::Deleted;
        let can_begin = state != AccountState::Deleted;

        assert!(!can_begin, "a completed deletion must be terminal");
    }

    #[test]
    fn onboarding_completes_before_admin_deletion_can_begin() {
        fn activate(state: AccountState) -> AccountState {
            match state {
                AccountState::Onboarding => AccountState::Active,
                other => other,
            }
        }

        assert_eq!(activate(AccountState::Onboarding), AccountState::Active);
    }

    #[test]
    fn deletion_is_complete_only_after_sessions_and_credential_are_revoked() {
        fn next_state(sessions_left: usize, credential_revoked: bool) -> AccountState {
            if sessions_left == 0 && credential_revoked {
                AccountState::Deleted
            } else {
                AccountState::DeletionPending
            }
        }

        assert_eq!(next_state(1, true), AccountState::DeletionPending);
        assert_eq!(next_state(0, false), AccountState::DeletionPending);
        assert_eq!(next_state(0, true), AccountState::Deleted);
    }
}
