use async_trait::async_trait;

use crate::domain::errors::AppError;
use crate::domain::models::{
    AllInclusiveQuoteHttpResponse, AllInclusiveQuoteRequest, AllInclusiveResortsHttpResponse,
    AuthenticatedSession, RciResortSearchHttpResponse, RciValidationHttpResponse,
    ResortSearchRequest,
};

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn get_available_session(&self) -> Result<Option<AuthenticatedSession>, AppError>;
}

#[async_trait]
pub trait RciGateway: Send + Sync {
    async fn validate_session(
        &self,
        session: &AuthenticatedSession,
    ) -> Result<RciValidationHttpResponse, AppError>;

    async fn search_resorts(
        &self,
        session: &AuthenticatedSession,
        request: &ResortSearchRequest,
    ) -> Result<RciResortSearchHttpResponse, AppError>;

    async fn list_all_inclusive_resorts(&self)
        -> Result<AllInclusiveResortsHttpResponse, AppError>;

    async fn quote_all_inclusive(
        &self,
        request: &AllInclusiveQuoteRequest,
    ) -> Result<AllInclusiveQuoteHttpResponse, AppError>;
}
