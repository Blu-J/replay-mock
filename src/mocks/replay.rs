use std::fs::File;

use async_trait::async_trait;

use crate::models::{DynamicBody, Replay, Request};

use super::RunMock;

/// We want to be able to replay from a set of replay sets, and the
/// first match means the first reply.
pub struct ReplayMock {
    replays: Vec<Replay>,
}
impl ReplayMock {
    /// Creating  a replay mock with a known set of replays
    pub fn new(replays: Vec<Replay>) -> Box<Self> {
        Box::new(Self { replays })
    }
    /// Creating  a replay mock with a known set of replays as a json file
    pub fn from_file(path: &str) -> Box<Self> {
        let file = File::open(path).expect("replay from file");
        let replays = serde_json::from_reader(&file).expect("parse replay");
        Self::new(replays)
    }
}
#[async_trait]
impl RunMock for ReplayMock {
    async fn run_mock(&self, request: &Request) -> Option<DynamicBody> {
        for replay in self.replays.iter() {
            if replay.matches_request(request) {
                return Some(replay.then.clone());
            }
        }
        None
    }
}
