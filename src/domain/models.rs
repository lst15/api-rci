use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedSession {
    pub username: String,
    pub headers: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RciValidationPayload {
    pub valid: bool,
    pub member_id: Option<String>,
    pub locale: Option<String>,
    pub expires_in: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RciValidationHttpResponse {
    pub status: u16,
    pub payload: Option<RciValidationPayload>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionValidation {
    pub valid: bool,
    pub username: String,
    pub member_id: Option<String>,
    pub locale: Option<String>,
    pub expires_in: Option<i64>,
    pub rci_status: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ResortSearchRequest {
    pub label: String,
    #[serde(rename = "minStartDate")]
    pub min_start_date: String,
    #[serde(rename = "maxStartDate")]
    pub max_start_date: String,
    #[serde(default)]
    pub filters: Option<Vec<String>>,
    #[serde(default)]
    pub from: Option<u32>,
    #[serde(default)]
    pub size: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RciResortSearchHttpResponse {
    pub status: u16,
    pub payload: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResortSearchResult {
    pub username: String,
    pub rci_status: u16,
    pub data: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllInclusiveResort {
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "UmbrellaCode")]
    pub umbrella_code: Option<String>,
    #[serde(rename = "UmbrellaName")]
    pub umbrella_name: Option<String>,
    #[serde(rename = "OnlyAdults")]
    pub only_adults: bool,
    #[serde(rename = "CurrencyCode")]
    pub currency_code: Option<String>,
    #[serde(rename = "Language")]
    pub language: Option<String>,
    #[serde(rename = "ContactNumbers")]
    pub contact_numbers: String,
    #[serde(rename = "HonorFee")]
    pub honor_fee: bool,
    #[serde(rename = "TaxesIncluded")]
    pub taxes_included: bool,
    #[serde(rename = "ContactEmail")]
    pub contact_email: Option<String>,
    #[serde(rename = "Status")]
    pub status: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AllInclusiveResortsHttpResponse {
    pub status: u16,
    pub payload: Option<Vec<AllInclusiveResort>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AllInclusiveQuoteRequest {
    #[serde(rename = "resortCode")]
    pub resort_code: String,
    #[serde(rename = "checkIn")]
    pub check_in: String,
    #[serde(rename = "checkOut")]
    pub check_out: String,
    #[serde(default, rename = "unitLabel")]
    pub unit_label: Option<String>,
    #[serde(default)]
    pub adults: Option<u32>,
    #[serde(default, rename = "childrenAges")]
    pub children_ages: Option<Vec<u32>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllInclusiveQuote {
    #[serde(rename = "resortCode")]
    pub resort_code: String,
    #[serde(rename = "checkIn")]
    pub check_in: String,
    #[serde(rename = "checkOut")]
    pub check_out: String,
    pub nights: u32,
    pub currency: String,
    #[serde(rename = "basePrice")]
    pub base_price: f64,
    #[serde(rename = "promotionalPrice")]
    pub promotional_price: Option<f64>,
    #[serde(rename = "prePayPromotionalPrice")]
    pub pre_pay_promotional_price: Option<f64>,
    #[serde(rename = "unitResortId")]
    pub unit_resort_id: u32,
    #[serde(rename = "unitName")]
    pub unit_name: String,
    #[serde(rename = "allInclusiveType")]
    pub all_inclusive_type: u32,
    #[serde(rename = "allInclusiveTypeName")]
    pub all_inclusive_type_name: String,
    #[serde(rename = "taxesIncluded")]
    pub taxes_included: Option<bool>,
    #[serde(rename = "honorFee")]
    pub honor_fee: Option<bool>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AllInclusiveQuoteHttpResponse {
    pub status: u16,
    pub payload: Option<AllInclusiveQuote>,
}
