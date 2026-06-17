use std::collections::HashMap;

use async_trait::async_trait;
use redis::AsyncCommands;

use crate::domain::errors::AppError;
use crate::domain::models::AuthenticatedSession;
use crate::domain::ports::SessionRepository;

pub struct RedisSessionRepository {
    client: redis::Client,
    session_index_key: String,
}

impl RedisSessionRepository {
    pub fn new(redis_url: String, session_index_key: String) -> Result<Self, AppError> {
        let client = redis::Client::open(redis_url)
            .map_err(|err| AppError::SessionStoreUnavailable(err.to_string()))?;

        Ok(Self {
            client,
            session_index_key,
        })
    }

    async fn connection(&self) -> Result<redis::aio::MultiplexedConnection, AppError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|err| AppError::SessionStoreUnavailable(err.to_string()))
    }
}

#[async_trait]
impl SessionRepository for RedisSessionRepository {
    async fn get_available_session(&self) -> Result<Option<AuthenticatedSession>, AppError> {
        let mut conn = self.connection().await?;
        let mut usernames: Vec<String> = conn
            .smembers(&self.session_index_key)
            .await
            .map_err(|err| AppError::SessionStoreUnavailable(err.to_string()))?;
        usernames.sort();

        for username in usernames {
            let payload: Option<String> = conn
                .get(&username)
                .await
                .map_err(|err| AppError::SessionStoreUnavailable(err.to_string()))?;

            let Some(payload) = payload else {
                let _: usize = conn
                    .srem(&self.session_index_key, &username)
                    .await
                    .map_err(|err| AppError::SessionStoreUnavailable(err.to_string()))?;
                continue;
            };

            let headers: HashMap<String, String> =
                serde_json::from_str(&payload).map_err(|err| AppError::InvalidCachedHeaders {
                    username: username.clone(),
                    reason: err.to_string(),
                })?;

            if headers.is_empty() {
                return Err(AppError::InvalidCachedHeaders {
                    username,
                    reason: "headers object is empty".to_string(),
                });
            }

            return Ok(Some(AuthenticatedSession { username, headers }));
        }

        Ok(None)
    }
}
