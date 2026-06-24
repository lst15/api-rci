use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;
use tracing::warn;

use crate::application::validate_session::RetryPolicy;
use crate::domain::errors::AppError;
use crate::domain::models::{ResortSearchRequest, ResortSearchResult};
use crate::domain::ports::{RciGateway, SessionRepository};

pub struct SearchResortsUseCase {
    session_repository: Arc<dyn SessionRepository>,
    rci_gateway: Arc<dyn RciGateway>,
    retry_policy: RetryPolicy,
}

impl SearchResortsUseCase {
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

    pub async fn execute(
        &self,
        request: ResortSearchRequest,
    ) -> Result<ResortSearchResult, AppError> {
        for attempt in 0..=self.retry_policy.attempts {
            let session = self
                .session_repository
                .get_available_session()
                .await?
                .ok_or(AppError::NoSessionAvailable)?;

            let response = self.rci_gateway.search_resorts(&session, &request).await?;
            if response.status == 401 || response.status == 403 {
                warn!(
                    username = %session.username,
                    status = response.status,
                    endpoint = "resorts_search",
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

            if !(200..300).contains(&response.status) {
                return Err(AppError::RciUnexpectedStatus {
                    status: response.status,
                });
            }

            let payload = response
                .payload
                .ok_or_else(|| AppError::RciUnavailable("missing resort search payload".into()))?;

            return Ok(ResortSearchResult {
                username: session.username,
                rci_status: response.status,
                data: payload,
            });
        }

        Err(AppError::NoSessionAvailable)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::json;

    use super::*;
    use crate::domain::models::{
        AllInclusiveQuoteHttpResponse, AllInclusiveQuoteRequest, AllInclusiveResortsHttpResponse,
        AuthenticatedSession, RciResortDetailsHttpResponse, RciResortPackagesHttpResponse,
        RciResortSearchHttpResponse, RciValidationHttpResponse, ResortDetailsRequest,
        ResortPackagesRequest,
    };

    struct FakeSessionRepository {
        sessions: Mutex<VecDeque<Option<AuthenticatedSession>>>,
    }

    #[async_trait]
    impl SessionRepository for FakeSessionRepository {
        async fn get_available_session(&self) -> Result<Option<AuthenticatedSession>, AppError> {
            Ok(self.sessions.lock().unwrap().pop_front().flatten())
        }
    }

    struct FakeRciGateway {
        responses: Mutex<VecDeque<RciResortSearchHttpResponse>>,
    }

    #[async_trait]
    impl RciGateway for FakeRciGateway {
        async fn validate_session(
            &self,
            _session: &AuthenticatedSession,
        ) -> Result<RciValidationHttpResponse, AppError> {
            unreachable!()
        }

        async fn search_resorts(
            &self,
            _session: &AuthenticatedSession,
            _request: &ResortSearchRequest,
        ) -> Result<RciResortSearchHttpResponse, AppError> {
            Ok(self.responses.lock().unwrap().pop_front().unwrap())
        }

        async fn resort_details(
            &self,
            _session: &AuthenticatedSession,
            _request: &ResortDetailsRequest,
        ) -> Result<RciResortDetailsHttpResponse, AppError> {
            unreachable!()
        }

        async fn resort_packages(
            &self,
            _session: &AuthenticatedSession,
            _request: &ResortPackagesRequest,
        ) -> Result<RciResortPackagesHttpResponse, AppError> {
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

    fn request() -> ResortSearchRequest {
        ResortSearchRequest {
            label: "Gramado".to_string(),
            min_start_date: "".to_string(),
            max_start_date: "".to_string(),
            filters: None,
            from: None,
            size: None,
        }
    }

    #[tokio::test]
    async fn returns_search_payload_without_headers() {
        let use_case = SearchResortsUseCase::new(
            Arc::new(FakeSessionRepository {
                sessions: Mutex::new(VecDeque::from([Some(session("Adrianop72"))])),
            }),
            Arc::new(FakeRciGateway {
                responses: Mutex::new(VecDeque::from([RciResortSearchHttpResponse {
                    status: 200,
                    payload: Some(json!({"availableResortCount": 18})),
                }])),
            }),
            RetryPolicy {
                attempts: 1,
                delay_seconds: 0,
            },
        );

        let result = use_case.execute(request()).await.unwrap();
        assert_eq!(result.username, "Adrianop72");
        assert_eq!(result.rci_status, 200);
        assert_eq!(result.data["availableResortCount"], 18);
    }

    #[tokio::test]
    async fn retries_auth_failure_for_search() {
        let use_case = SearchResortsUseCase::new(
            Arc::new(FakeSessionRepository {
                sessions: Mutex::new(VecDeque::from([
                    Some(session("stale")),
                    Some(session("fresh")),
                ])),
            }),
            Arc::new(FakeRciGateway {
                responses: Mutex::new(VecDeque::from([
                    RciResortSearchHttpResponse {
                        status: 401,
                        payload: None,
                    },
                    RciResortSearchHttpResponse {
                        status: 200,
                        payload: Some(json!({"availableResortCount": 18})),
                    },
                ])),
            }),
            RetryPolicy {
                attempts: 1,
                delay_seconds: 0,
            },
        );

        let result = use_case.execute(request()).await.unwrap();
        assert_eq!(result.username, "fresh");
    }
}
