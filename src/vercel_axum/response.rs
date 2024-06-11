use lambda_http::http::header::{HeaderMap, HeaderValue};
use lambda_http::Body;
use serde_derive::Serialize;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EventResponse {
    pub(crate) status_code: u16,
    #[serde(
        skip_serializing_if = "HeaderMap::is_empty",
        with = "http_serde::header_map"
    )]
    pub(crate) headers: HeaderMap<HeaderValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) body: Option<Body>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) encoding: Option<String>,
}
