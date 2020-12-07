#![deny(missing_docs, warnings)]
//! ### Purpose
//! We want to to capture a proxy, and replay, and even pass it through if needed.
use std::{mem::replace, net::SocketAddr, sync::Arc};

use futures::channel::oneshot;
use futures::lock::Mutex;
pub use hyper;
use hyper::{
    header, http,
    service::{make_service_fn, service_fn},
};
use hyper::{Body, Method as HyperMethod, Request, Response, Server, StatusCode};
use models::Method;
use serde_json::Value;

/// Mocks are the ways that we can create route mocking
/// There are usefull tools like gateway, which is a proxy
/// and replay that can replay a json
pub mod mocks;
/// Models are the abstraction so that way we can simplify the types
/// to the closure mock, and abstract out to any implmentation.
pub mod models;

type RunMock = Box<dyn mocks::RunMock + Send + Sync>;

type Mocks = Arc<Mutex<Vec<Arc<RunMock>>>>;

/// Mock Server is the main piece, this will start a server on a random port
/// and we can get the port and url. We then can modify behaviour with the mocks.
pub struct MockServer {
    mocks: Mocks,
    /// Address where the server is hosting.
    pub address: SocketAddr,
    kill: Option<oneshot::Sender<()>>,
}
async fn router(mocks: Mocks, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let path = match req.uri().path_and_query() {
        Some(path_query) => format!("{}", path_query),
        _ => return Ok(not_found()),
    };
    let method = match *req.method() {
        HyperMethod::POST => Method::Post,
        HyperMethod::PUT => Method::Put,
        HyperMethod::GET => Method::Get,
        HyperMethod::DELETE => Method::Delete,
        _ => return Ok(not_found()),
    };
    let whole_body = hyper::body::to_bytes(req.into_body()).await?;
    let body = if whole_body.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&whole_body) {
            Ok(body) => body,
            Err(_err) => {
                return Ok(bad_request(format!(
                    "Can't parse {:?} as valid json",
                    whole_body
                )))
            }
        }
    };
    let request = models::Request { method, path, body };
    let mocks = mocks.lock().await.clone();
    for mock in mocks.iter() {
        let mock_result = mock.run_mock(&request).await;
        if let Some(result) = mock_result {
            let body = match serde_json::to_string(&result) {
                Ok(a) => a,
                Err(_err) => {
                    return Ok(internal_error(format!(
                        "Can't convert {:?} into bytes",
                        result
                    )))
                }
            };
            if result == Value::Null {
                return Ok(Response::builder().body(Body::empty()).unwrap());
            }
            return Ok(Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap());
        }
    }
    Ok(not_found())
}

fn not_found() -> Response<Body> {
    let mut not_found = Response::default();
    *not_found.status_mut() = StatusCode::NOT_FOUND;
    not_found
}
fn bad_request(message: String) -> Response<Body> {
    let body = serde_json::to_string(&message).unwrap();
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .status(http::StatusCode::BAD_REQUEST)
        .body(Body::from(body))
        .unwrap()
}
fn internal_error(message: String) -> Response<Body> {
    let body = serde_json::to_string(&message).unwrap();
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(body))
        .unwrap()
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let kill = replace(&mut self.kill, None);
        if let Some(kill) = kill {
            kill.send(())
                .expect("Sending kill signal for cleanup of mock server");
        }
    }
}

impl MockServer {
    /// Notes: Creating a on a random port
    pub async fn new() -> MockServer {
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
            kill: Some(s),
        }
    }

    /// Use this to change the behaviour of the server, adding in a replay.
    pub async fn mock(self, mock: RunMock) -> Self {
        self.mocks.lock().await.push(Arc::new(mock));
        self
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::remove_file, time::Instant};

    use crate::{
        mocks::Gateway,
        mocks::{ClosureMock, FactoryClosure, ReplayMock},
        MockServer,
    };
    use futures::{
        channel::{mpsc, oneshot},
        SinkExt, StreamExt,
    };
    use serde_json::{json, Value};
    use tokio::{self, task};

    #[tokio::test]
    async fn capture_and_replay() {
        let file_path = "/tmp/test.json";
        let client = reqwest::Client::new();
        let body_one: Value = {
            let mock = MockServer::new()
                .await
                .mock(Gateway::new_replay(
                    "",
                    "https://cat-fact.herokuapp.com",
                    file_path,
                ))
                .await;
            let url = format!("http://{}/facts", mock.address);

            let res = client.get(&url).send().await.expect("Valid get");
            res.json().await.expect("Serde")
        };

        assert_ne!(body_one, json!(null));

        let body_two: Value = {
            let mock = MockServer::new()
                .await
                .mock(ReplayMock::from_file(file_path))
                .await;
            let url = format!("http://{}/facts", mock.address);
            let res = client.get(&url).send().await.expect("Valid get");
            res.json().await.expect("Serde")
        };

        assert_eq!(body_one, body_two);
        remove_file(file_path).expect("Remove the file for the testing");
    }

    #[tokio::test]
    async fn closure_test() {
        let mock = MockServer::new()
            .await
            .mock(ClosureMock::new(|_req| async { Some(json!("Good")) }))
            .await;
        let url = format!("http://{}/facts", mock.address);
        let client = reqwest::Client::new();

        let res = client.get(&url).send().await.expect("Valid get");

        let body_one: Value = res.json().await.expect("Serde");

        assert_eq!(body_one, json!("Good"));
    }

    #[tokio::test]
    async fn async_test() {
        let (send_one, mut rec_one) = mpsc::channel::<oneshot::Sender<Value>>(1);
        let (send_two, mut rec_two) = mpsc::channel::<oneshot::Sender<Value>>(1);
        let mock = MockServer::new()
            .await
            .mock(FactoryClosure::new(move || {
                let mut send_one = send_one.clone();
                |req| async move {
                    if &req.path == "/one" {
                        let (send, rec) = oneshot::channel::<Value>();
                        send_one.send(send).await.unwrap();
                        let value = rec.await.unwrap();
                        return Some(value);
                    }
                    None
                }
            }))
            .await
            .mock(FactoryClosure::new(move || {
                let mut send_two = send_two.clone();
                |req| async move {
                    if &req.path == "/two" {
                        let (send, rec) = oneshot::channel::<Value>();
                        send_two.send(send).await.unwrap();
                        let value = rec.await.unwrap();
                        return Some(value);
                    }
                    None
                }
            }))
            .await;
        let address = mock.address;
        let body_one = task::spawn(async move {
            let url = format!("http://{}/one", address);

            let client = reqwest::Client::new();

            let body: Value = client
                .get(&url)
                .send()
                .await
                .expect("get")
                .json()
                .await
                .expect("json");

            assert_eq!(body, json!("one"));
            Instant::now()
        });
        let body_two = task::spawn(async move {
            let url = format!("http://{}/two", address);

            let client = reqwest::Client::new();

            let body: Value = client
                .get(&url)
                .send()
                .await
                .expect("get")
                .json()
                .await
                .expect("json");

            assert_eq!(body, json!("two"));
            Instant::now()
        });

        rec_two.next().await.unwrap().send(json!("two")).unwrap();
        rec_one.next().await.unwrap().send(json!("one")).unwrap();
        let body_one = body_one.await.unwrap();
        let body_two = body_two.await.unwrap();
        assert!(body_one > body_two);
    }
}
