use async_trait::async_trait;
use core::future::Future;

use crate::models::{DynamicBody, Request};

use super::RunMock;

/// For when we want to make a custom matcher for the mocking, we
/// can use a simple closure.
pub struct ClosureMock<
    V: Into<DynamicBody>,
    T: Future<Output = Option<V>> + Sync + Send,
    F: Fn(Request) -> T + Sync + Send,
> {
    closure: F,
}
impl<
        V: Into<DynamicBody>,
        T: Future<Output = Option<V>> + Sync + Send,
        F: Fn(Request) -> T + Sync + Send,
    > ClosureMock<V, T, F>
{
    ///
    pub fn new(closure: F) -> Box<ClosureMock<V, T, F>> {
        Box::new(Self { closure })
    }
}
#[async_trait]
impl<
        V: Into<DynamicBody>,
        T: Future<Output = Option<V>> + Sync + Send,
        F: Fn(Request) -> T + Sync + Send,
    > RunMock for ClosureMock<V, T, F>
{
    async fn run_mock(&self, request: &Request) -> Option<DynamicBody> {
        let response = (self.closure)(request.clone()).await;
        Some(response?.into())
    }
}
