use async_trait::async_trait;
use futures::Future;
use serde_json::Value;

use crate::models::Request;

use super::RunMock;

/// We want to have a function that creates a runner
pub struct FactoryClosure<
    T: Future<Output = Option<Value>> + Sync + Send,
    F: Fn() -> R + Sync + Send,
    R: FnOnce(Request) -> T + Sync + Send,
> {
    closure: F,
}
impl<
        T: Future<Output = Option<Value>> + Sync + Send,
        F: Fn() -> R + Sync + Send,
        R: FnOnce(Request) -> T + Sync + Send,
    > FactoryClosure<T, F, R>
{
    ///
    pub fn new(closure: F) -> Box<FactoryClosure<T, F, R>> {
        Box::new(Self { closure })
    }
}
#[async_trait]
impl<
        T: Future<Output = Option<Value>> + Sync + Send,
        F: Fn() -> R + Sync + Send,
        R: FnOnce(Request) -> T + Sync + Send,
    > RunMock for FactoryClosure<T, F, R>
{
    async fn run_mock(&self, request: &Request) -> Option<Value> {
        let response = (self.closure)()(request.clone()).await;
        Some(response?)
    }
}
