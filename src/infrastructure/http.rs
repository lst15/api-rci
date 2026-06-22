use std::time::Duration;

use async_trait::async_trait;
use http::header::{HeaderName, HeaderValue};
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::domain::errors::AppError;
use crate::domain::models::{
    AllInclusiveQuote, AllInclusiveQuoteHttpResponse, AllInclusiveQuoteRequest, AllInclusiveResort,
    AllInclusiveResortsHttpResponse, AuthenticatedSession, RciResortSearchHttpResponse,
    RciValidationHttpResponse, RciValidationPayload, ResortSearchRequest,
};
use crate::domain::ports::RciGateway;

const HOP_BY_HOP_HEADERS: &[&str] = &[
    "connection",
    "content-length",
    "host",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

pub struct ReqwestRciGateway {
    client: reqwest::Client,
    validation_url: String,
    typeahead_url: String,
    resort_search_url: String,
    all_inclusive_resorts_url: String,
    all_inclusive_unit_types_url: String,
    all_inclusive_types_url: String,
    all_inclusive_billing_details_url: String,
}

impl ReqwestRciGateway {
    pub fn new(
        validation_url: String,
        typeahead_url: String,
        resort_search_url: String,
        all_inclusive_resorts_url: String,
        all_inclusive_unit_types_url: String,
        all_inclusive_types_url: String,
        all_inclusive_billing_details_url: String,
        timeout_seconds: u64,
    ) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;

        Ok(Self {
            client,
            validation_url,
            typeahead_url,
            resort_search_url,
            all_inclusive_resorts_url,
            all_inclusive_unit_types_url,
            all_inclusive_types_url,
            all_inclusive_billing_details_url,
        })
    }

    async fn resolve_destination_filter(
        &self,
        session: &AuthenticatedSession,
        request: &ResortSearchRequest,
        customer_id: &str,
    ) -> Result<Option<String>, AppError> {
        let mut url = reqwest::Url::parse(&self.typeahead_url)
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;
        url.query_pairs_mut()
            .append_pair("Ntt", &format!("{}*", request.label.trim()))
            .append_pair("consumerChannelQ", "Web")
            .append_pair("memberTypeQ", "WEEKS")
            .append_pair("operatorIdQ", "WEB00USER")
            .append_pair("customerIdQ", customer_id);

        let response = self
            .client
            .get(url)
            .headers(build_rci_json_header_map(session)?)
            .send()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;
        let status = response.status().as_u16();

        if status == 401 || status == 403 {
            return Err(AppError::RciAuthFailed { status });
        }
        if !(200..300).contains(&status) {
            return Err(AppError::RciUnexpectedStatus { status });
        }

        let body = response
            .text()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;
        let payload = serde_json::from_str::<Value>(&body).map_err(|err| {
            AppError::RciUnavailable(format!(
                "invalid typeahead response: {err}; body={}",
                body.chars().take(500).collect::<String>()
            ))
        })?;

        Ok(find_destination_filter(&payload, &request.label))
    }
}

#[async_trait]
impl RciGateway for ReqwestRciGateway {
    async fn validate_session(
        &self,
        session: &AuthenticatedSession,
    ) -> Result<RciValidationHttpResponse, AppError> {
        let headers = build_header_map(session)?;
        let response = self
            .client
            .get(&self.validation_url)
            .headers(headers)
            .send()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;

        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            return Ok(RciValidationHttpResponse {
                status,
                payload: None,
            });
        }

        if !(200..300).contains(&status) {
            return Ok(RciValidationHttpResponse {
                status,
                payload: None,
            });
        }

        let payload = response
            .json::<RawValidationPayload>()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;

        Ok(RciValidationHttpResponse {
            status,
            payload: Some(payload.into_domain()),
        })
    }

    async fn search_resorts(
        &self,
        session: &AuthenticatedSession,
        request: &ResortSearchRequest,
    ) -> Result<RciResortSearchHttpResponse, AppError> {
        let customer_id = extract_customer_id(session)?;
        let resolved_filter = if request
            .filters
            .as_ref()
            .is_some_and(|filters| !filters.is_empty())
        {
            None
        } else {
            Some(
                self.resolve_destination_filter(session, request, &customer_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::RciUnavailable(format!(
                            "destination not found for label {}",
                            request.label
                        ))
                    })?,
            )
        };
        let headers = build_rci_json_header_map(session)?;
        let url = format!(
            "{}?customerIdQ={}&appType=rcio&consumerChannelQ=Web&memberTypeQ=WEEKS&operatorIdQ=WEB00USER",
            self.resort_search_url, customer_id
        );
        let body = build_resort_search_body(request, &customer_id, resolved_filter);

        let response = self
            .client
            .post(url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;

        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            return Ok(RciResortSearchHttpResponse {
                status,
                payload: None,
            });
        }

        if !(200..300).contains(&status) {
            return Ok(RciResortSearchHttpResponse {
                status,
                payload: None,
            });
        }

        let payload = response
            .json::<Value>()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;

        Ok(RciResortSearchHttpResponse {
            status,
            payload: Some(payload),
        })
    }

    async fn list_all_inclusive_resorts(
        &self,
    ) -> Result<AllInclusiveResortsHttpResponse, AppError> {
        let response = self
            .client
            .get(&self.all_inclusive_resorts_url)
            .headers(build_all_inclusive_header_map())
            .send()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Ok(AllInclusiveResortsHttpResponse {
                status,
                payload: None,
            });
        }

        let payload = response
            .json::<Vec<AllInclusiveResort>>()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;

        Ok(AllInclusiveResortsHttpResponse {
            status,
            payload: Some(payload),
        })
    }

    async fn quote_all_inclusive(
        &self,
        request: &AllInclusiveQuoteRequest,
    ) -> Result<AllInclusiveQuoteHttpResponse, AppError> {
        let resort_code = request.resort_code.trim();
        if resort_code.is_empty() {
            return Err(AppError::RciUnavailable("missing resort code".into()));
        }

        let unit_types_url = format!(
            "{}?resortCode={}&date=null",
            self.all_inclusive_unit_types_url, resort_code
        );
        let unit_types_response = self
            .client
            .get(unit_types_url)
            .headers(build_all_inclusive_header_map())
            .send()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;
        let unit_status = unit_types_response.status().as_u16();
        if !(200..300).contains(&unit_status) {
            return Ok(AllInclusiveQuoteHttpResponse {
                status: unit_status,
                payload: None,
            });
        }
        let unit_types = unit_types_response
            .json::<Vec<RawAllInclusiveUnitType>>()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;
        let unit_type = select_unit_type(&unit_types, request.unit_label.as_deref())
            .ok_or_else(|| AppError::RciUnavailable("no all-inclusive unit type found".into()))?;

        let ai_types_url = format!(
            "{}?resortCode={}&date=null",
            self.all_inclusive_types_url, resort_code
        );
        let ai_types_response = self
            .client
            .get(ai_types_url)
            .headers(build_all_inclusive_header_map())
            .send()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;
        let ai_status = ai_types_response.status().as_u16();
        if !(200..300).contains(&ai_status) {
            return Ok(AllInclusiveQuoteHttpResponse {
                status: ai_status,
                payload: None,
            });
        }
        let ai_types = ai_types_response
            .json::<Vec<RawAllInclusiveType>>()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;
        let ai_type = ai_types
            .first()
            .ok_or_else(|| AppError::RciUnavailable("no all-inclusive type found".into()))?;

        let check_in = parse_iso_date(&request.check_in)
            .ok_or_else(|| AppError::RciUnavailable("invalid check-in date".into()))?;
        let check_out = parse_iso_date(&request.check_out)
            .ok_or_else(|| AppError::RciUnavailable("invalid check-out date".into()))?;
        let today = current_local_date_string();
        let adults = request.adults.unwrap_or(2).max(1);
        let mut ages = vec![30; adults as usize];
        if let Some(children_ages) = &request.children_ages {
            ages.extend(children_ages.iter().copied());
        }

        let body = json!({
            "ConfirmationDateString": null,
            "PrePaymentDateString": today,
            "CheckInDateString": format_ai_date(check_in),
            "CheckOutDateString": format_ai_date(check_out),
            "Ages": ages,
            "ResortCode": resort_code,
            "UnitResortId": unit_type.id,
            "AllInclusiveType": ai_type.id,
            "MembersCountry": null,
            "Language": "en",
            "MemberId": null,
            "QuoteTimeStampString": null,
            "ChannelCode": null,
            "MarketId": null,
            "QuoteIp": null,
            "Callcenter": null,
            "InferredCountry": null,
            "RequireHonorFee": null
        });

        let billing_response = self
            .client
            .post(&self.all_inclusive_billing_details_url)
            .headers(build_all_inclusive_json_header_map())
            .json(&body)
            .send()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;
        let status = billing_response.status().as_u16();
        if !(200..300).contains(&status) {
            return Ok(AllInclusiveQuoteHttpResponse {
                status,
                payload: None,
            });
        }
        let billing = billing_response
            .json::<RawBillingDetails>()
            .await
            .map_err(|err| AppError::RciUnavailable(err.to_string()))?;

        Ok(AllInclusiveQuoteHttpResponse {
            status,
            payload: Some(AllInclusiveQuote {
                resort_code: resort_code.to_string(),
                check_in: request.check_in.clone(),
                check_out: request.check_out.clone(),
                nights: billing
                    .cant_nights
                    .unwrap_or_else(|| nights_between(check_in, check_out)),
                currency: billing
                    .currency_symbol_detail
                    .unwrap_or_else(|| "USD".to_string()),
                base_price: billing.base_price.unwrap_or(0.0),
                promotional_price: billing.promotional_price,
                pre_pay_promotional_price: billing.pre_pay_promotional_price,
                unit_resort_id: unit_type.id,
                unit_name: unit_type.name.clone(),
                all_inclusive_type: ai_type.id,
                all_inclusive_type_name: ai_type.name.clone(),
                taxes_included: billing
                    .fees
                    .as_ref()
                    .and_then(|fees| fees.first())
                    .map(|fee| fee.tax > 0.0),
                honor_fee: billing
                    .fees
                    .as_ref()
                    .and_then(|fees| fees.first())
                    .map(|fee| fee.honours_fee),
                error: empty_string_to_none(billing.error),
            }),
        })
    }
}

fn build_header_map(session: &AuthenticatedSession) -> Result<HeaderMap, AppError> {
    let mut map = HeaderMap::new();

    for (name, value) in &session.headers {
        if HOP_BY_HOP_HEADERS.contains(&name.to_ascii_lowercase().as_str()) {
            continue;
        }

        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|err| {
            AppError::InvalidCachedHeaders {
                username: session.username.clone(),
                reason: format!("invalid header name {name}: {err}"),
            }
        })?;
        let header_value =
            HeaderValue::from_str(value).map_err(|err| AppError::InvalidCachedHeaders {
                username: session.username.clone(),
                reason: format!("invalid header value for {name}: {err}"),
            })?;

        map.insert(header_name, header_value);
    }

    Ok(map)
}

fn build_rci_json_header_map(session: &AuthenticatedSession) -> Result<HeaderMap, AppError> {
    let mut map = build_header_map(session)?;
    map.insert(
        http::header::ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    map.insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain"),
    );
    map.insert(
        http::header::ORIGIN,
        HeaderValue::from_static("https://www.rci.com"),
    );
    map.insert(
        http::header::REFERER,
        HeaderValue::from_static("https://www.rci.com/"),
    );
    map.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static("same-site"),
    );
    map.insert(
        HeaderName::from_static("sec-fetch-mode"),
        HeaderValue::from_static("cors"),
    );
    map.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("empty"),
    );
    map.insert(
        HeaderName::from_static("priority"),
        HeaderValue::from_static("u=1, i"),
    );
    Ok(map)
}

fn build_all_inclusive_header_map() -> HeaderMap {
    let mut map = HeaderMap::new();
    map.insert(
        http::header::USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) api-rci/0.1"),
    );
    map.insert(
        http::header::ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    map.insert(
        http::header::ACCEPT_LANGUAGE,
        HeaderValue::from_static("pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7"),
    );
    map.insert(
        http::header::REFERER,
        HeaderValue::from_static("https://ai.rci.com/"),
    );
    map.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("empty"),
    );
    map.insert(
        HeaderName::from_static("sec-fetch-mode"),
        HeaderValue::from_static("cors"),
    );
    map.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static("same-origin"),
    );
    map
}

fn build_all_inclusive_json_header_map() -> HeaderMap {
    let mut map = build_all_inclusive_header_map();
    map.insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    map
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SimpleDate {
    year: i32,
    month: u32,
    day: u32,
}

fn parse_iso_date(value: &str) -> Option<SimpleDate> {
    let date = value.trim().split('T').next()?.split(' ').next()?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(SimpleDate { year, month, day })
}

fn format_ai_date(date: SimpleDate) -> String {
    format!("{}/{}/{}", date.day, date.month, date.year)
}

fn current_local_date_string() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| (duration.as_secs() / 86_400) as i64)
        .unwrap_or(0);
    format_ai_date(civil_from_days(days))
}

fn civil_from_days(days_since_epoch: i64) -> SimpleDate {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };

    SimpleDate {
        year: year as i32,
        month: month as u32,
        day: day as u32,
    }
}

fn days_from_civil(date: SimpleDate) -> i64 {
    let mut year = date.year as i64;
    let month = date.month as i64;
    let day = date.day as i64;
    year -= if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month_prime + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn nights_between(check_in: SimpleDate, check_out: SimpleDate) -> u32 {
    (days_from_civil(check_out) - days_from_civil(check_in)).max(1) as u32
}

fn select_unit_type<'a>(
    units: &'a [RawAllInclusiveUnitType],
    unit_label: Option<&str>,
) -> Option<&'a RawAllInclusiveUnitType> {
    let normalized_label = unit_label.map(normalize_unit_label).unwrap_or_default();
    if normalized_label.is_empty() {
        return units.first();
    }

    units
        .iter()
        .find(|unit| normalize_unit_label(&unit.name).contains(&normalized_label))
        .or_else(|| {
            units.iter().find(|unit| {
                let normalized_name = normalize_unit_label(&unit.name);
                (normalized_label.contains("studio") && normalized_name.contains("studio"))
                    || (normalized_label.contains("hotel") && normalized_name.contains("hotel"))
                    || (normalized_label.contains("1 bedroom")
                        && normalized_name.contains("1 bedroom"))
                    || (normalized_label.contains("2 bedroom")
                        && normalized_name.contains("2 bedroom"))
            })
        })
        .or_else(|| units.first())
}

fn normalize_unit_label(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    if lower.contains("est") || lower.contains("studio") {
        return "studio".to_string();
    }
    if lower.contains("hotel") || lower.contains("hoteleira") {
        return "hotel".to_string();
    }
    if lower.contains("1 quarto") || lower.contains("one bedroom") || lower.contains("1 bedroom") {
        return "1 bedroom".to_string();
    }
    if lower.contains("2 quarto") || lower.contains("two bedroom") || lower.contains("2 bedroom") {
        return "2 bedroom".to_string();
    }
    lower
}

fn build_resort_search_body(
    request: &ResortSearchRequest,
    customer_id: &str,
    resolved_filter: Option<String>,
) -> Value {
    let filters = request
        .filters
        .clone()
        .filter(|filters| !filters.is_empty())
        .or_else(|| resolved_filter.map(|filter| vec![filter]))
        .unwrap_or_default();
    let from = request.from.unwrap_or(0);
    let size = request.size.unwrap_or(16);

    json!({
        "language": "PT",
        "locale": "pt_BR",
        "numberOfResortsToReturn": 0,
        "productType": "All",
        "productTypes": [],
        "recordRollup": false,
        "filters": filters,
        "membershipType": "",
        "searchCriteria": {
            "distanceUnit": "km",
            "minStartDate": request.min_start_date,
            "maxStartDate": request.max_start_date,
            "onSaleInventory": false,
            "preferredCheckInDate": "",
            "resortSearchCriteria": [],
            "youChoseFilters": [],
            "minLoS": 1,
            "maxLoS": 7,
            "offerId": "",
            "preview": false,
            "latitude": null,
            "longitude": null,
            "googleGeoSearch": null,
            "label": request.label,
            "wsid": "",
            "radius": null
        },
        "pointsExchangeSearchType": "ALL",
        "selectedDeposit": "",
        "showNonBookable": false,
        "pagination": {
            "from": from,
            "size": size
        },
        "customerId": customer_id,
        "visibleNightlyRentalInv": false,
        "searchNightlyRentalInvSelected": false,
        "selectedDepositTP": ""
    })
}

fn find_destination_filter(payload: &Value, requested_label: &str) -> Option<String> {
    let normalized_requested = normalize_destination_label(requested_label);
    let search_group = payload.get("searchGroup").unwrap_or(payload);
    let groups = ["rciDictValues", "rciValues", "geocodeValues"];
    let mut fallback = None;

    for group in groups {
        let Some(items) = search_group.get(group).and_then(Value::as_array) else {
            continue;
        };

        for item in items {
            let type_name = item
                .get("typeName")
                .or_else(|| item.get("searchType"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !type_name.eq_ignore_ascii_case("Dimension") {
                continue;
            }

            let Some(value) = item
                .get("value")
                .or_else(|| item.get("xmlId"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let label = item
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default();

            if normalize_destination_label(label) == normalized_requested {
                return Some(value.to_string());
            }
            if fallback.is_none() {
                fallback = Some(value.to_string());
            }
        }
    }

    fallback
}

fn normalize_destination_label(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn extract_customer_id(session: &AuthenticatedSession) -> Result<String, AppError> {
    let cookie = session
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        .map(|(_, value)| value.as_str())
        .unwrap_or_default();

    if let Some(value) = extract_cookie_value(cookie, "__attentive_client_user_id") {
        if !value.is_empty() {
            return Ok(value);
        }
    }

    if let Some(value) = extract_cookie_value(cookie, "USER_INFO") {
        if let Some(member_id) = extract_member_id_from_user_info_cookie(&value) {
            return Ok(member_id);
        }
    }

    Err(AppError::MissingCustomerId {
        username: session.username.clone(),
    })
}

fn extract_cookie_value(cookie: &str, key: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let trimmed = part.trim();
        let (name, value) = trimmed.split_once('=')?;
        if name == key {
            Some(value.trim_matches('"').to_string())
        } else {
            None
        }
    })
}

fn extract_member_id_from_user_info_cookie(value: &str) -> Option<String> {
    let decoded = percent_decode(value);
    let data: UserInfoCookie = serde_json::from_str(&decoded).ok()?;
    data.member_id
}

fn percent_decode(value: &str) -> String {
    let mut output = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    output.push(byte);
                    index += 3;
                    continue;
                }
            }
        }

        if bytes[index] == b'+' {
            output.push(b' ');
        } else {
            output.push(bytes[index]);
        }
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

#[derive(Debug, Deserialize)]
struct RawValidationPayload {
    #[serde(default)]
    valid: String,
    #[serde(default, rename = "memberId")]
    member_id: Option<String>,
    #[serde(default)]
    locale: Option<String>,
    #[serde(default)]
    expires_in: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAllInclusiveUnitType {
    #[serde(rename = "Id")]
    id: u32,
    #[serde(rename = "Name")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct RawAllInclusiveType {
    #[serde(rename = "Id")]
    id: u32,
    #[serde(rename = "Name")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct RawBillingFee {
    #[serde(default, rename = "HonoursFee")]
    honours_fee: bool,
    #[serde(default, rename = "Tax")]
    tax: f64,
}

#[derive(Debug, Deserialize)]
struct RawBillingDetails {
    #[serde(default, rename = "Fees")]
    fees: Option<Vec<RawBillingFee>>,
    #[serde(default, rename = "BasePrice")]
    base_price: Option<f64>,
    #[serde(default, rename = "PromotionalPrice")]
    promotional_price: Option<f64>,
    #[serde(default, rename = "PrePayPromotionalPrice")]
    pre_pay_promotional_price: Option<f64>,
    #[serde(default, rename = "CurrencySymbolDetail")]
    currency_symbol_detail: Option<String>,
    #[serde(default, rename = "CantNights")]
    cant_nights: Option<u32>,
    #[serde(default, rename = "Error")]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoCookie {
    #[serde(rename = "memberId")]
    member_id: Option<String>,
}

impl RawValidationPayload {
    fn into_domain(self) -> RciValidationPayload {
        RciValidationPayload {
            valid: self.valid.eq_ignore_ascii_case("true"),
            member_id: empty_string_to_none(self.member_id),
            locale: empty_string_to_none(self.locale),
            expires_in: self.expires_in.and_then(|value| value.parse::<i64>().ok()),
        }
    }
}

fn empty_string_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|value| if value.is_empty() { None } else { Some(value) })
}
