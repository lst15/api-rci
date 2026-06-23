use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub api_host: String,
    pub api_port: u16,
    pub redis_url: String,
    pub session_index_key: String,
    pub rci_validation_url: String,
    pub rci_resort_search_url: String,
    pub rci_all_inclusive_resorts_url: String,
    pub rci_all_inclusive_unit_types_url: String,
    pub rci_all_inclusive_types_url: String,
    pub rci_all_inclusive_billing_details_url: String,
    pub auth_retry_attempts: u32,
    pub auth_retry_delay_seconds: u64,
    pub request_timeout_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            api_host: env_string("API_HOST", "0.0.0.0"),
            api_port: env_parse("API_PORT", 8080)?,
            redis_url: env_string("REDIS_URL", "redis://localhost:6379/0"),
            session_index_key: env_string("RCI_SESSION_INDEX_KEY", "rci:sessions:active"),
            rci_validation_url: env_string(
                "RCI_VALIDATION_URL",
                "https://services.b2c.rci.com/ext/v1/jwt/validation",
            ),
            rci_resort_search_url: env_string(
                "RCI_RESORT_SEARCH_URL",
                "https://services.b2c.rci.com/ext/v1/resort-operations/v1/resorts/weeks/list-view",
            ),
            rci_all_inclusive_resorts_url: env_string(
                "RCI_ALL_INCLUSIVE_RESORTS_URL",
                "https://ai.rci.com/api/Data/GetResorts",
            ),
            rci_all_inclusive_unit_types_url: env_string(
                "RCI_ALL_INCLUSIVE_UNIT_TYPES_URL",
                "https://ai.rci.com/api/Data/GetResortUnitTypes",
            ),
            rci_all_inclusive_types_url: env_string(
                "RCI_ALL_INCLUSIVE_TYPES_URL",
                "https://ai.rci.com/api/Data/GetTypeAiResort",
            ),
            rci_all_inclusive_billing_details_url: env_string(
                "RCI_ALL_INCLUSIVE_BILLING_DETAILS_URL",
                "https://ai.rci.com/api/Data/GetBillingDetails",
            ),
            auth_retry_attempts: env_parse("RCI_AUTH_RETRY_ATTEMPTS", 1)?,
            auth_retry_delay_seconds: env_parse("RCI_AUTH_RETRY_DELAY_SECONDS", 5)?,
            request_timeout_seconds: env_parse("RCI_REQUEST_TIMEOUT_SECONDS", 30)?,
        })
    }
}

fn env_string(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn env_parse<T>(name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
{
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("invalid value for {name}: {value}")),
        Err(_) => Ok(default),
    }
}
