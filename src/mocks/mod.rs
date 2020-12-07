use async_trait::async_trait;
use serde_json::Value;

use crate::models::Request;
mod closure;
mod factory_closure;
mod gateway;
mod replay;

pub use closure::*;
pub use factory_closure::*;
pub use gateway::*;
pub use replay::*;
#[async_trait]
/// Want to test a route to see if this mock works, hence the option.
/// When there is a value it expects that we are using this mock and stops here.
pub trait RunMock {
    ///
    async fn run_mock(&self, request: &Request) -> Option<Value>;
}
