use async_trait::async_trait;
use serde_json::Value;

use crate::models::Request;
mod closure;
mod gateway;
mod replay;

pub use closure::*;
pub use gateway::*;
pub use replay::*;
#[async_trait]
pub trait RunMock {
    async fn run_mock(&self, request: &Request) -> Option<Value>;
}
