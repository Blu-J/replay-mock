use async_trait::async_trait;

use crate::models::{DynamicBody, Request};
use core::future::Future;

use super::RunMock;

/// We want to have a function that creates a runner
pub struct FactoryClosure<
    V: Into<DynamicBody>,
    T: Future<Output = Option<V>> + Sync + Send,
    F: Fn() -> R + Sync + Send,
    R: FnOnce(Request) -> T + Sync + Send,
> {
    closure: F,
}
impl<
        V: Into<DynamicBody>,
        T: Future<Output = Option<V>> + Sync + Send,
        F: Fn() -> R + Sync + Send,
        R: FnOnce(Request) -> T + Sync + Send,
    > FactoryClosure<V, T, F, R>
{
    ///
    pub fn new(closure: F) -> Box<FactoryClosure<V, T, F, R>> {
        Box::new(Self { closure })
    }
}
#[async_trait]
impl<
        V: Into<DynamicBody>,
        T: Future<Output = Option<V>> + Sync + Send,
        F: Fn() -> R + Sync + Send,
        R: FnOnce(Request) -> T + Sync + Send,
    > RunMock for FactoryClosure<V, T, F, R>
{
    async fn run_mock(&self, request: &Request) -> Option<DynamicBody> {
        let response = (self.closure)()(request.clone()).await;
        Some(response?.into())
    }
}
