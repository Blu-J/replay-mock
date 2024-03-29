use std::{fs::File, io::Write, sync::Mutex, time::Duration};

use async_trait::async_trait;
use tracing::warn;

use crate::models::{DynamicBody, Method, Replay, Request};

use super::RunMock;

/// Gateway is a proxy to another server. And when we get a response,
/// We capture that in a value, so if we have a file name on deletion we create a replay
/// for the replay mock
pub struct Gateway {
    path: String,
    uri: String,
    file: Option<String>,
    replays: Mutex<Vec<Replay>>,
}
impl Gateway {
    /// Create a simple proxy server
    pub fn new(path: &str, uri: &str) -> Box<Self> {
        Box::new(Self {
            path: path.to_string(),
            uri: uri.to_string(),
            file: None,
            replays: Default::default(),
        })
    }
    /// Create a proxy server that on death will create a replay file
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
    async fn run_mock(&self, request: &Request) -> Option<DynamicBody> {
        let path = request.path.strip_prefix(&self.path)?;
        println!("{:?}", request);

        let uri = format!(
            "{}{}{}",
            self.uri,
            path,
            request
                .queries
                .clone()
                .map(|x| format!("?{}", x))
                .unwrap_or_default()
        );
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60 * 5))
            .build()
            .ok()?;
        let response = match request.method {
            Method::Post => client.post(&uri),
            Method::Put => client.put(&uri),
            Method::Get => client.get(&uri),
            Method::Delete => client.delete(&uri),
            Method::Head => client.head(&uri),
            Method::Patch => client.patch(&uri),
            Method::Trace | Method::Connect | Method::Options | Method::Other => return None,
        };
        let response = match &request.body {
            &None => response,
            Some(DynamicBody::Text(body)) => response.body(body.clone()),
            Some(DynamicBody::Bytes(body)) => response.body(body.clone()),
            Some(DynamicBody::Json(body)) => response
                .header("content-type", "application/json")
                .json(&body),
        };

        let response = response.send().await.ok()?;
        println!("response {:?}", response);
        // And then, if the request gets a response...
        if response.status() != 200 {
            warn!("Error status: {}", response.status());
            return None;
        }
        let body_bytes = response.bytes().await.ok()?;
        let response_body: DynamicBody = if let Some(json) =
            serde_json::from_slice(&body_bytes).ok()
        {
            DynamicBody::Json(json)
        } else if let Some(body) = String::from_utf8(body_bytes.iter().cloned().collect()).ok() {
            DynamicBody::Text(body)
        } else {
            DynamicBody::Bytes(body_bytes.into_iter().collect())
        };
        {
            let mut replays = self.replays.lock().ok()?;
            replays.push(Replay {
                when: request.clone(),
                then: response_body.clone(),
            });
        }

        Some(response_body)
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
