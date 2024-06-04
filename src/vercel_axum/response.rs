use lambda_http::http::header::{HeaderMap, HeaderValue};
use lambda_http::Body;
use serde::ser::{Error as SerError, SerializeMap, Serializer};
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

fn serialize_headers<S>(headers: &HeaderMap<HeaderValue>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = serializer.serialize_map(Some(headers.keys_len()))?;
    for key in headers.keys() {
        let map_values = headers
            .get_all(key)
            .into_iter()
            .map(HeaderValue::to_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(S::Error::custom)?;

        for v in &map_values {
            map.serialize_entry(key.as_str(), v)?;
        }
    }
    map.end()
}
