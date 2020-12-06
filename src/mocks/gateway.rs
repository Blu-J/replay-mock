use std::{fs::File, io::Write, sync::Mutex};

use async_trait::async_trait;
use bytes::buf::BufExt;
use hyper::{header, Body, Client};
use serde_json::Value;

use crate::models::{Replay, Request};

use super::RunMock;

pub struct Gateway {
    path: String,
    uri: String,
    file: Option<String>,
    replays: Mutex<Vec<Replay>>,
}
impl Gateway {
    pub fn new(path: &str, uri: &str) -> Box<Self> {
        Box::new(Self {
            path: path.to_string(),
            uri: uri.to_string(),
            file: None,
            replays: Default::default(),
        })
    }
    pub fn new_replay(path: &str, uri: &str, file: &str) -> Box<Self> {
        Box::new(Self {
            path: path.to_string(),
            uri: uri.to_string(),
            file: Some(file.to_string()),
            replays: Default::default(),
        })
    }
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
            .request(
                hyper::Request::builder()
                    .method(method)
                    .header(header::CONTENT_TYPE, "application/body")
                    .uri(uri)
                    .body(Body::from(body))
                    .expect("request builder"),
            )
            .await
            .ok()?;

        let body = hyper::body::aggregate(res).await.ok()?;
        let body: Value = serde_json::from_reader(body.reader()).ok()?;
        {
            let mut replays = self.replays.lock().ok()?;
            replays.push(Replay {
                when: request.clone(),
                then: body.clone(),
            });
        }

        Some(body)
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            let replays = self.replays.lock().expect("getting replays");
            if replays.is_empty() {
                return;
            }
            let replays_bytes = serde_json::to_vec(&*replays).expect("Serializing the replays");
            let mut file = File::create(file).expect("creating gateway file");
            file.write_all(&replays_bytes)
                .expect("writing gateway to file");
        }
    }
}
