use std::sync::Arc;

use crate::domain::errors::AppError;
use crate::domain::models::{AllInclusiveQuote, AllInclusiveQuoteRequest};
use crate::domain::ports::RciGateway;

pub struct QuoteAllInclusiveUseCase {
    rci_gateway: Arc<dyn RciGateway>,
}

impl QuoteAllInclusiveUseCase {
    pub fn new(rci_gateway: Arc<dyn RciGateway>) -> Self {
        Self { rci_gateway }
    }

    pub async fn execute(
        &self,
        request: AllInclusiveQuoteRequest,
    ) -> Result<AllInclusiveQuote, AppError> {
        let response = self.rci_gateway.quote_all_inclusive(&request).await?;
        if !(200..300).contains(&response.status) {
            return Err(AppError::RciUnexpectedStatus {
                status: response.status,
            });
        }

        response
            .payload
            .ok_or_else(|| AppError::RciUnavailable("missing all-inclusive quote payload".into()))
    }
}
