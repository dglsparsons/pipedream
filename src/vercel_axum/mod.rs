use axum::response::IntoResponse;
use http_body_util::BodyExt;
use lambda_runtime::LambdaEvent;
use std::{future::Future, pin::Pin};
use tower::Layer;
use tower_service::Service;

mod request;
mod response;

use request::{VercelEvent, VercelRequest};
use response::EventResponse;

#[derive(Clone, Copy)]
pub struct VercelLayer;

impl<S> Layer<S> for VercelLayer {
    type Service = VercelService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        VercelService { inner }
    }
}

pub struct VercelService<S> {
    inner: S,
}

type Event<'a> = LambdaEvent<VercelEvent<'a>>;

impl<S> Service<Event<'_>> for VercelService<S>
where
    S: Service<axum::http::Request<axum::body::Body>>,
    S::Response: axum::response::IntoResponse + Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Send + 'static,
{
    type Response = EventResponse;
    type Error = lambda_http::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, event: Event) -> Self::Future {
        let (event, _context) = event.into_parts();
        let request = serde_json::from_str::<VercelRequest>(&event.body).unwrap();

        let mut builder = axum::http::request::Builder::new()
            .method(request.method)
            .uri(format!("{}{}", request.host, request.path));
        for (key, value) in request.headers {
            if let Some(k) = key {
                builder = builder.header(k, value);
            }
        }

        let request: axum::http::Request<axum::body::Body> = match request.body {
            None => builder.body(axum::body::Body::default()).unwrap(),
            Some(b) => builder.body(axum::body::Body::from(b)).unwrap(),
        };

        let fut = self.inner.call(request);
        let fut = async move {
            let resp = fut.await?;
            let (parts, body) = resp.into_response().into_parts();
            let bytes = body.into_data_stream().collect().await?.to_bytes();
            let bytes: &[u8] = &bytes;
            let body = std::str::from_utf8(bytes).unwrap();
            let body: Option<lambda_http::Body> = match body {
                "" => None,
                _ => Some(lambda_http::Body::from(body)),
            };
            Ok(EventResponse {
                status_code: parts.status.as_u16(),
                body,
                headers: parts.headers,
                encoding: None,
            })
        };

        Box::pin(fut)
    }
}