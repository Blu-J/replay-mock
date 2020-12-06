use async_trait::async_trait;
use bytes::buf::BufExt;
use hyper::{header, Body, Client};
use serde_json::Value;

use crate::models::Request;

use super::RunMock;
pub struct Gateway {
    pub path: String,
    pub uri: String,
}
#[async_trait]
impl RunMock for Gateway {
    async fn run_mock(&self, request: &Request) -> Option<Value> {
        let path = request.path.strip_prefix(&self.path)?;
        let https = hyper_rustls::HttpsConnector::new();

        let client = Client::builder().build(https);

        let uri: hyper::Uri = format!("{}{}", self.uri, path).parse().ok()?;
        let body = serde_json::to_string(&request.body).ok()?;
        let method: hyper::Method = request.method.as_method_string().parse().ok()?;

        let res = client
            .request(dbg!(hyper::Request::builder()
                .method(method)
                .header(header::CONTENT_TYPE, "application/body")
                .uri(uri)
                .body(Body::from(body))
                .expect("request builder")))
            .await
            .ok()?;

        let body = hyper::body::aggregate(res).await.ok()?;

        Some((serde_json::from_reader(body.reader()).ok()?))
    }
}
