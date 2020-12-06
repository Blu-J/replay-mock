use std::fs::File;

use async_trait::async_trait;
use serde_json::Value;

use crate::models::{Replay, Request};

use super::RunMock;

pub struct ReplayMock {
    replays: Vec<Replay>,
}
impl ReplayMock {
    pub fn new(replays: Vec<Replay>) -> Box<Self> {
        Box::new(Self { replays })
    }
    pub fn from_file(path: &str) -> Box<Self> {
        let file = File::open(path).expect("replay from file");
        let replays = serde_json::from_reader(&file).expect("parse replay");
        Self::new(replays)
    }
}
#[async_trait]
impl RunMock for ReplayMock {
    async fn run_mock(&self, request: &Request) -> Option<Value> {
        for replay in self.replays.iter() {
            if replay.matches_request(request) {
                return Some(replay.then.clone());
            }
        }
        None
    }
}
