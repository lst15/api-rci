use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;
use tracing::warn;

use crate::application::validate_session::RetryPolicy;
use crate::domain::errors::AppError;
use crate::domain::models::{ResortDetailsRequest, ResortDetailsResult};
use crate::domain::ports::{RciGateway, SessionRepository};

pub struct GetResortDetailsUseCase {
    session_repository: Arc<dyn SessionRepository>,
    rci_gateway: Arc<dyn RciGateway>,
    retry_policy: RetryPolicy,
}

impl GetResortDetailsUseCase {
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
        request: ResortDetailsRequest,
    ) -> Result<ResortDetailsResult, AppError> {
        if request.resort_code.trim().is_empty() {
            return Err(AppError::RciUnavailable("missing resort code".into()));
        }

        for attempt in 0..=self.retry_policy.attempts {
            let session = self
                .session_repository
                .get_available_session()
                .await?
                .ok_or(AppError::NoSessionAvailable)?;

            let response = self.rci_gateway.resort_details(&session, &request).await?;
            if response.status == 401 || response.status == 403 {
                warn!(
                    username = %session.username,
                    status = response.status,
                    endpoint = "resort_details",
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
                .ok_or_else(|| AppError::RciUnavailable("missing resort details payload".into()))?;

            return Ok(ResortDetailsResult {
                username: session.username,
                rci_status: response.status,
                data: payload,
            });
        }

        Err(AppError::NoSessionAvailable)
    }
}
