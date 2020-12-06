use async_trait::async_trait;
use serde_json::Value;

use crate::models::Request;
#[async_trait]
pub trait RunMock {
    async fn run_mock(&self, request: &Request) -> Option<Value>;
}
