use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;
use tracing::warn;

use crate::domain::errors::AppError;
use crate::domain::models::{RciValidationHttpResponse, SessionValidation};
use crate::domain::ports::{RciGateway, SessionRepository};

#[derive(Clone, Copy, Debug)]
pub struct RetryPolicy {
    pub attempts: u32,
    pub delay_seconds: u64,
}

pub struct ValidateSessionUseCase {
    session_repository: Arc<dyn SessionRepository>,
    rci_gateway: Arc<dyn RciGateway>,
    retry_policy: RetryPolicy,
}

impl ValidateSessionUseCase {
    pub fn new(
        session_repository: Arc<dyn SessionRepository>,
        rci_gateway: Arc<dyn RciGateway>,
        retry_policy: RetryPolicy,
    ) -> Self {
        Self {
            session_repository,
            rci_gateway,
            retry_policy,
        }
    }

    pub async fn execute(&self) -> Result<SessionValidation, AppError> {
        for attempt in 0..=self.retry_policy.attempts {
            let session = self
                .session_repository
                .get_available_session()
                .await?
                .ok_or(AppError::NoSessionAvailable)?;

            let response = self.rci_gateway.validate_session(&session).await?;
            if is_auth_error(response.status) {
                warn!(
                    username = %session.username,
                    status = response.status,
                    endpoint = "session_validation",
                    attempt = attempt + 1,
                    max_attempts = self.retry_policy.attempts + 1,
                    "RCI authentication/authorization failure"
                );

                if attempt < self.retry_policy.attempts {
                    sleep(Duration::from_secs(self.retry_policy.delay_seconds)).await;
                    continue;
                }

                return Err(AppError::RciAuthFailed {
                    status: response.status,
                });
            }

            return normalize_validation_response(session.username, response);
        }

        Err(AppError::NoSessionAvailable)
    }
}

fn is_auth_error(status: u16) -> bool {
    status == 401 || status == 403
}

fn normalize_validation_response(
    username: String,
    response: RciValidationHttpResponse,
) -> Result<SessionValidation, AppError> {
    if !(200..300).contains(&response.status) {
        return Err(AppError::RciUnexpectedStatus {
            status: response.status,
        });
    }

    let payload = response
        .payload
        .ok_or_else(|| AppError::RciUnavailable("missing validation payload".to_string()))?;

    Ok(SessionValidation {
        valid: payload.valid,
        username,
        member_id: payload.member_id,
        locale: payload.locale,
        expires_in: payload.expires_in,
        rci_status: response.status,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::domain::models::{
        AllInclusiveQuoteHttpResponse, AllInclusiveQuoteRequest, AllInclusiveResortsHttpResponse,
        AuthenticatedSession, RciResortSearchHttpResponse, RciValidationPayload,
        ResortSearchRequest,
    };

    struct FakeSessionRepository {
        sessions: Mutex<VecDeque<Option<AuthenticatedSession>>>,
    }

    impl FakeSessionRepository {
        fn new(sessions: Vec<Option<AuthenticatedSession>>) -> Self {
            Self {
                sessions: Mutex::new(VecDeque::from(sessions)),
            }
        }
    }

    #[async_trait]
    impl SessionRepository for FakeSessionRepository {
        async fn get_available_session(&self) -> Result<Option<AuthenticatedSession>, AppError> {
            Ok(self.sessions.lock().unwrap().pop_front().flatten())
        }
    }

    struct FakeRciGateway {
        responses: Mutex<VecDeque<RciValidationHttpResponse>>,
    }

    impl FakeRciGateway {
        fn new(responses: Vec<RciValidationHttpResponse>) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
            }
        }
    }

    #[async_trait]
    impl RciGateway for FakeRciGateway {
        async fn validate_session(
            &self,
            _session: &AuthenticatedSession,
        ) -> Result<RciValidationHttpResponse, AppError> {
            Ok(self.responses.lock().unwrap().pop_front().unwrap())
        }

        async fn search_resorts(
            &self,
            _session: &AuthenticatedSession,
            _request: &ResortSearchRequest,
        ) -> Result<RciResortSearchHttpResponse, AppError> {
            unreachable!()
        }

        async fn list_all_inclusive_resorts(
            &self,
        ) -> Result<AllInclusiveResortsHttpResponse, AppError> {
            unreachable!()
        }

        async fn quote_all_inclusive(
            &self,
            _request: &AllInclusiveQuoteRequest,
        ) -> Result<AllInclusiveQuoteHttpResponse, AppError> {
            unreachable!()
        }
    }

    fn session(username: &str) -> AuthenticatedSession {
        AuthenticatedSession {
            username: username.to_string(),
            headers: HashMap::from([("Cookie".to_string(), "a=b".to_string())]),
        }
    }

    fn ok_response() -> RciValidationHttpResponse {
        RciValidationHttpResponse {
            status: 200,
            payload: Some(RciValidationPayload {
                valid: true,
                member_id: Some("DB8103299".to_string()),
                locale: Some("pt_BR".to_string()),
                expires_in: Some(1799),
            }),
        }
    }

    #[tokio::test]
    async fn returns_no_session_when_repository_is_empty() {
        let use_case = ValidateSessionUseCase::new(
            Arc::new(FakeSessionRepository::new(vec![None])),
            Arc::new(FakeRciGateway::new(vec![])),
            RetryPolicy {
                attempts: 1,
                delay_seconds: 0,
            },
        );

        assert!(matches!(
            use_case.execute().await,
            Err(AppError::NoSessionAvailable)
        ));
    }

    #[tokio::test]
    async fn normalizes_successful_validation_response() {
        let use_case = ValidateSessionUseCase::new(
            Arc::new(FakeSessionRepository::new(vec![Some(session(
                "Adrianop72",
            ))])),
            Arc::new(FakeRciGateway::new(vec![ok_response()])),
            RetryPolicy {
                attempts: 1,
                delay_seconds: 0,
            },
        );

        let result = use_case.execute().await.unwrap();
        assert_eq!(result.username, "Adrianop72");
        assert_eq!(result.valid, true);
        assert_eq!(result.member_id.as_deref(), Some("DB8103299"));
        assert_eq!(result.rci_status, 200);
    }

    #[tokio::test]
    async fn retries_auth_failure_with_fresh_headers() {
        let use_case = ValidateSessionUseCase::new(
            Arc::new(FakeSessionRepository::new(vec![
                Some(session("first")),
                Some(session("second")),
            ])),
            Arc::new(FakeRciGateway::new(vec![
                RciValidationHttpResponse {
                    status: 401,
                    payload: None,
                },
                ok_response(),
            ])),
            RetryPolicy {
                attempts: 1,
                delay_seconds: 0,
            },
        );

        let result = use_case.execute().await.unwrap();
        assert_eq!(result.username, "second");
    }

    #[tokio::test]
    async fn returns_auth_failed_when_retry_does_not_fix_auth_failure() {
        let use_case = ValidateSessionUseCase::new(
            Arc::new(FakeSessionRepository::new(vec![
                Some(session("first")),
                Some(session("second")),
            ])),
            Arc::new(FakeRciGateway::new(vec![
                RciValidationHttpResponse {
                    status: 403,
                    payload: None,
                },
                RciValidationHttpResponse {
                    status: 403,
                    payload: None,
                },
            ])),
            RetryPolicy {
                attempts: 1,
                delay_seconds: 0,
            },
        );

        assert!(matches!(
            use_case.execute().await,
            Err(AppError::RciAuthFailed { status: 403 })
        ));
    }
}
