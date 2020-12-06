use std::{cell::Cell, net::SocketAddr, net::UdpSocket, sync::Arc};

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::{lock::Mutex, TryStreamExt};
pub use hyper;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};

type Mocks = Arc<Mutex<Vec<Box<dyn RunMock + Send + Sync>>>>;

pub struct MockServer {
    pub mocks: Mocks,
    pub address: SocketAddr,
    kill: oneshot::Sender<()>,
}
async fn router(mocks: Mocks, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    for mock in mocks.lock().await.iter() {
        let mock_result = mock.run_mock(&req).await?;
        if let Some(result) = mock_result {
            return Ok(result);
        }
    }

    let mut not_found = Response::default();
    *not_found.status_mut() = StatusCode::NOT_FOUND;
    Ok(not_found)
}

impl MockServer {
    async fn new() -> MockServer {
        let addr: SocketAddr = ([0, 0, 0, 0], 0).into();
        let mocks: Mocks = Default::default();

        let service = {
            let mocks = mocks.clone();
            make_service_fn(move |_| {
                let mocks = mocks.clone();
                async move {
                    Ok::<_, hyper::Error>(service_fn(move |req| {
                        let mocks = mocks.clone();
                        async move { router(mocks, req).await }
                    }))
                }
            })
        };

        let (send_address, address_receiver) = oneshot::channel();
        let (s, r) = oneshot::channel();
        tokio::spawn(async move {
            let server = Server::bind(&addr).serve(service);
            send_address.send(server.local_addr()).unwrap();
            server
                .with_graceful_shutdown(async { r.await.unwrap() })
                .await
                .expect("Running server");
        });
        MockServer {
            mocks,
            address: address_receiver.await.unwrap(),
            kill: s,
        }
    }

    async fn mock(&mut self, mock: Box<dyn RunMock + Sync + Send>) {
        self.mocks.lock().await.push(mock);
    }
}

#[async_trait]
pub trait RunMock {
    async fn run_mock(
        &self,
        request: &Request<Body>,
    ) -> Result<Option<Response<Body>>, hyper::Error>;
}

#[cfg(test)]
mod tests {
    use crate::MockServer;
    use hyper::Client;
    use serde_json::{json, Value};
    use tokio::{self};

    #[tokio::test]
    async fn it_works() {
        let mock = MockServer::new().await;
        let url = dbg!(format!("http://{}/facts", mock.address));

        let client = Client::new();

        let res = client
            .get(url.parse().expect("valid url"))
            .await
            .expect("Valid get");

        // And then, if the request gets a response...
        println!("status: {}", res.status());

        // Concatenate the body stream into a single buffer...
        let body: Value =
            serde_json::from_slice(&hyper::body::to_bytes(res).await.expect("as bytes"))
                .expect("Serde");
        mock.kill.send(()).unwrap();

        assert_eq!(body, json!({}));
    }
}
