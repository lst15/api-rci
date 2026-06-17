use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

use crate::application::list_all_inclusive_resorts::ListAllInclusiveResortsUseCase;
use crate::application::quote_all_inclusive::QuoteAllInclusiveUseCase;
use crate::application::search_resorts::SearchResortsUseCase;
use crate::application::validate_session::ValidateSessionUseCase;
use crate::domain::models::{
    AllInclusiveQuote, AllInclusiveQuoteRequest, AllInclusiveResort, ResortSearchRequest,
    ResortSearchResult,
};
use crate::interfaces::http::errors::ApiError;

#[derive(Clone)]
pub struct AppState {
    validate_session_use_case: Arc<ValidateSessionUseCase>,
    search_resorts_use_case: Arc<SearchResortsUseCase>,
    list_all_inclusive_resorts_use_case: Arc<ListAllInclusiveResortsUseCase>,
    quote_all_inclusive_use_case: Arc<QuoteAllInclusiveUseCase>,
}

pub fn build_router(
    validate_session_use_case: Arc<ValidateSessionUseCase>,
    search_resorts_use_case: Arc<SearchResortsUseCase>,
    list_all_inclusive_resorts_use_case: Arc<ListAllInclusiveResortsUseCase>,
    quote_all_inclusive_use_case: Arc<QuoteAllInclusiveUseCase>,
) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/session/validate", get(validate_session))
        .route("/v1/resorts/search", post(search_resorts))
        .route(
            "/v1/resorts/all-inclusive/mandatory",
            get(list_all_inclusive_resorts),
        )
        .route("/v1/all-inclusive/quote", post(quote_all_inclusive))
        .with_state(AppState {
            validate_session_use_case,
            search_resorts_use_case,
            list_all_inclusive_resorts_use_case,
            quote_all_inclusive_use_case,
        })
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn validate_session(
    State(state): State<AppState>,
) -> Result<Json<SessionValidationResponse>, ApiError> {
    let validation = state.validate_session_use_case.execute().await?;
    Ok(Json(SessionValidationResponse {
        valid: validation.valid,
        username: validation.username,
        member_id: validation.member_id,
        locale: validation.locale,
        expires_in: validation.expires_in,
        rci_status: validation.rci_status,
    }))
}

async fn search_resorts(
    State(state): State<AppState>,
    Json(request): Json<ResortSearchRequest>,
) -> Result<Json<ResortSearchResult>, ApiError> {
    let result = state.search_resorts_use_case.execute(request).await?;
    Ok(Json(result))
}

async fn list_all_inclusive_resorts(
    State(state): State<AppState>,
) -> Result<Json<Vec<AllInclusiveResort>>, ApiError> {
    let result = state.list_all_inclusive_resorts_use_case.execute().await?;
    Ok(Json(result))
}

async fn quote_all_inclusive(
    State(state): State<AppState>,
    Json(request): Json<AllInclusiveQuoteRequest>,
) -> Result<Json<AllInclusiveQuote>, ApiError> {
    let result = state.quote_all_inclusive_use_case.execute(request).await?;
    Ok(Json(result))
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
}

#[derive(Debug, Serialize)]
struct SessionValidationResponse {
    valid: bool,
    username: String,
    member_id: Option<String>,
    locale: Option<String>,
    expires_in: Option<i64>,
    rci_status: u16,
}
