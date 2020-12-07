use std::{fs::File, io::Write, sync::Mutex};

use async_trait::async_trait;
use serde_json::Value;
use tracing::warn;

use crate::models::{Method, Replay, Request};

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
    async fn run_mock(&self, request: &Request) -> Option<Value> {
        let path = request.path.strip_prefix(&self.path)?;

        let uri = format!("{}{}", self.uri, path);
        let client = reqwest::Client::new();
        let response = match request.method {
            Method::Post => client.post(&uri),
            Method::Put => client.put(&uri),
            Method::Get => client.get(&uri),
            Method::Delete => client.delete(&uri),
        };
        let response = dbg!(
            match &request.body {
                &Value::Null => response,
                body => response.json(body),
            }
            .send()
            .await
        )
        .ok()?;

        // And then, if the request gets a response...
        if response.status() != 200 {
            warn!("Error status: {}", response.status());
            return None;
        }
        let response_body: Value = response.json().await.ok()?;
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
