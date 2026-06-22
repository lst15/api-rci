mod application;
mod domain;
mod infrastructure;
mod interfaces;

use std::net::SocketAddr;
use std::sync::Arc;

use application::list_all_inclusive_resorts::ListAllInclusiveResortsUseCase;
use application::quote_all_inclusive::QuoteAllInclusiveUseCase;
use application::search_resorts::SearchResortsUseCase;
use application::validate_session::{RetryPolicy, ValidateSessionUseCase};
use infrastructure::config::Config;
use infrastructure::http::ReqwestRciGateway;
use infrastructure::redis_session::RedisSessionRepository;
use interfaces::http::routes::build_router;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env()?;
    let session_repository = Arc::new(RedisSessionRepository::new(
        config.redis_url.clone(),
        config.session_index_key.clone(),
    )?);
    let rci_gateway = Arc::new(ReqwestRciGateway::new(
        config.rci_validation_url.clone(),
        config.rci_typeahead_url.clone(),
        config.rci_resort_search_url.clone(),
        config.rci_all_inclusive_resorts_url.clone(),
        config.rci_all_inclusive_unit_types_url.clone(),
        config.rci_all_inclusive_types_url.clone(),
        config.rci_all_inclusive_billing_details_url.clone(),
        config.request_timeout_seconds,
    )?);
    let retry_policy = RetryPolicy {
        attempts: config.auth_retry_attempts,
        delay_seconds: config.auth_retry_delay_seconds,
    };
    let validate_session_use_case = Arc::new(ValidateSessionUseCase::new(
        session_repository.clone(),
        rci_gateway.clone(),
        retry_policy,
    ));
    let search_resorts_use_case = Arc::new(SearchResortsUseCase::new(
        session_repository,
        rci_gateway.clone(),
        retry_policy,
    ));
    let list_all_inclusive_resorts_use_case =
        Arc::new(ListAllInclusiveResortsUseCase::new(rci_gateway.clone()));
    let quote_all_inclusive_use_case = Arc::new(QuoteAllInclusiveUseCase::new(rci_gateway));

    let app = build_router(
        validate_session_use_case,
        search_resorts_use_case,
        list_all_inclusive_resorts_use_case,
        quote_all_inclusive_use_case,
    );
    let addr: SocketAddr = format!("{}:{}", config.api_host, config.api_port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!(%addr, "api-rci started");
    axum::serve(listener, app).await?;

    Ok(())
}
