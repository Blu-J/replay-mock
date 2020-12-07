use async_trait::async_trait;
use futures::Future;
use serde_json::Value;

use crate::models::Request;

use super::RunMock;

/// For when we want to make a custom matcher for the mocking, we
/// can use a simple closure.
pub struct ClosureMock<
    T: Future<Output = Option<Value>> + Sync + Send,
    F: Fn(&Request) -> T + Sync + Send,
> {
    closure: F,
}
impl<T: Future<Output = Option<Value>> + Sync + Send, F: Fn(&Request) -> T + Sync + Send>
    ClosureMock<T, F>
{
    ///
    pub fn new(closure: F) -> Box<ClosureMock<T, F>> {
        Box::new(Self { closure: closure })
    }
}
#[async_trait]
impl<T: Future<Output = Option<Value>> + Sync + Send, F: Fn(&Request) -> T + Sync + Send> RunMock
    for ClosureMock<T, F>
{
    async fn run_mock(&self, request: &Request) -> Option<Value> {
        let response = (self.closure)(request).await;
        Some(response?)
    }
}
