use std::sync::Arc;

use crate::domain::errors::AppError;
use crate::domain::models::AllInclusiveResort;
use crate::domain::ports::RciGateway;

pub struct ListAllInclusiveResortsUseCase {
    rci_gateway: Arc<dyn RciGateway>,
}

impl ListAllInclusiveResortsUseCase {
    pub fn new(rci_gateway: Arc<dyn RciGateway>) -> Self {
        Self { rci_gateway }
    }

    pub async fn execute(&self) -> Result<Vec<AllInclusiveResort>, AppError> {
        let response = self.rci_gateway.list_all_inclusive_resorts().await?;
        if !(200..300).contains(&response.status) {
            return Err(AppError::RciUnexpectedStatus {
                status: response.status,
            });
        }

        response
            .payload
            .ok_or_else(|| AppError::RciUnavailable("missing all-inclusive resorts payload".into()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::domain::models::{
        AllInclusiveQuoteHttpResponse, AllInclusiveQuoteRequest, AllInclusiveResortsHttpResponse,
        AuthenticatedSession, RciResortDetailsHttpResponse, RciResortPackagesHttpResponse,
        RciResortSearchHttpResponse, RciValidationHttpResponse, ResortDetailsRequest,
        ResortPackagesRequest, ResortSearchRequest,
    };

    struct FakeRciGateway {
        responses: Mutex<VecDeque<AllInclusiveResortsHttpResponse>>,
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
            unreachable!()
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
            Ok(self.responses.lock().unwrap().pop_front().unwrap())
        }

        async fn quote_all_inclusive(
            &self,
            _request: &AllInclusiveQuoteRequest,
        ) -> Result<AllInclusiveQuoteHttpResponse, AppError> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn returns_all_inclusive_resorts() {
        let use_case = ListAllInclusiveResortsUseCase::new(Arc::new(FakeRciGateway {
            responses: Mutex::new(VecDeque::from([AllInclusiveResortsHttpResponse {
                status: 200,
                payload: Some(vec![AllInclusiveResort {
                    code: "0131".to_string(),
                    name: "CONDOVAC LA COSTA - 0131".to_string(),
                    description: None,
                    umbrella_code: None,
                    umbrella_name: None,
                    only_adults: false,
                    currency_code: None,
                    language: None,
                    contact_numbers: "".to_string(),
                    honor_fee: false,
                    taxes_included: true,
                    contact_email: Some("jcorea@condovac.com".to_string()),
                    status: 1,
                }]),
            }])),
        }));

        let result = use_case.execute().await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, "0131");
        assert_eq!(result[0].taxes_included, true);
    }
}
