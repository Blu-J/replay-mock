#![deny(missing_docs)]
//! ### Purpose
//! We want to to capture a proxy, and replay, and even pass it through if needed.
use std::{
    mem::replace,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use models::{DynamicBody, Method};
use serde_json::Value;
use tokio::sync::oneshot;
use tracing::warn;
use warp::{filters, Filter};

/// Mocks are the ways that we can create route mocking
/// There are usefull tools like gateway, which is a proxy
/// and replay that can replay a json
pub mod mocks;
/// Models are the abstraction so that way we can simplify the types
/// to the closure mock, and abstract out to any implmentation.
pub mod models;

type RunMock = Box<dyn mocks::RunMock + Send + Sync>;

type Mocks = Arc<Mutex<Vec<Arc<RunMock>>>>;

#[derive(Debug, Clone)]
enum ResultType {
    Ok { value: DynamicBody },
    NotFound,
}

/// Mock Server is the main piece, this will start a server on a random port
/// and we can get the port and url. We then can modify behaviour with the mocks.
pub struct MockServer {
    mocks: Mocks,
    /// Address where the server is hosting.
    pub address: SocketAddr,
    kill: Option<oneshot::Sender<()>>,
}
async fn router(
    mocks: Mocks,
    path: String,
    queries: Option<String>,
    method: Method,
    body: Option<DynamicBody>,
) -> ResultType {
    let request = models::Request {
        method,
        queries,
        path,
        body,
    };
    let mocks = mocks.lock().unwrap().clone();
    for mock in mocks.iter() {
        let mock_result = mock.run_mock(&request).await;
        if let Some(value) = mock_result {
            return ResultType::Ok { value };
        }
    }
    ResultType::NotFound
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let kill = replace(&mut self.kill, None);
        if let Some(kill) = kill {
            kill.send(())
                .expect("Sending kill signal for cleanup of mock server");
            // self.server_task.
        }
    }
}

fn with_sendable<V: Clone + Send + Sync>(
    value: V,
) -> impl Filter<Extract = (V,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || value.clone())
}
async fn route(
    mocks: Mocks,
    path: warp::filters::path::FullPath,
    queries: Option<String>,
    body: Option<DynamicBody>,
    method: warp::http::Method,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    let method: Method = match method {
        warp::http::Method::OPTIONS => Method::Options,
        warp::http::Method::PATCH => Method::Patch,
        warp::http::Method::POST => Method::Post,
        warp::http::Method::PUT => Method::Put,
        warp::http::Method::TRACE => Method::Trace,
        warp::http::Method::HEAD => Method::Head,
        warp::http::Method::GET => Method::Get,
        warp::http::Method::DELETE => Method::Delete,
        warp::http::Method::CONNECT => Method::Connect,
        _ => Method::Other,
    };
    let path = path.as_str();
    let routed = router(mocks, path.to_string(), queries, method, body.clone()).await;
    match routed {
        ResultType::Ok {
            value: DynamicBody::Json(value),
        } => Ok(Box::new(warp::reply::json(&value))),
        ResultType::Ok {
            value: DynamicBody::Text(text),
        } => Ok(Box::new(warp::hyper::Response::builder().body(text))),
        ResultType::Ok {
            value: DynamicBody::Bytes(bytes),
        } => Ok(Box::new(warp::hyper::Response::builder().body(bytes))),
        ResultType::NotFound => {
            warn!(
                "\"Can't find route {:?}@{} with body {:?} \"",
                method, path, body,
            );
            Err(warp::reject::not_found())
        }
    }
}
async fn json_body_route(
    mocks: Mocks,
    path: warp::filters::path::FullPath,
    queries: String,
    body: Value,
    method: warp::http::Method,
) -> Result<impl warp::Reply, warp::Rejection> {
    route(
        mocks,
        path,
        Some(queries),
        Some(DynamicBody::Json(body)),
        method,
    )
    .await
}
async fn bytes_body_route(
    mocks: Mocks,
    path: warp::filters::path::FullPath,
    queries: String,
    body: bytes::Bytes,
    method: warp::http::Method,
) -> Result<impl warp::Reply, warp::Rejection> {
    route(
        mocks,
        path,
        Some(queries),
        Some(DynamicBody::Bytes(body.into_iter().collect())),
        method,
    )
    .await
}
async fn json_body_route_no_queries(
    mocks: Mocks,
    path: warp::filters::path::FullPath,
    body: Value,
    method: warp::http::Method,
) -> Result<impl warp::Reply, warp::Rejection> {
    route(mocks, path, None, Some(DynamicBody::Json(body)), method).await
}
async fn no_body_route(
    mocks: Mocks,
    path: warp::filters::path::FullPath,
    queries: String,
    method: warp::http::Method,
) -> Result<impl warp::Reply, warp::Rejection> {
    route(mocks, path, Some(queries), None, method).await
}
async fn no_body_route_no_queries(
    mocks: Mocks,
    path: warp::filters::path::FullPath,
    method: warp::http::Method,
) -> Result<impl warp::Reply, warp::Rejection> {
    route(mocks, path, None, None, method).await
}

impl MockServer {
    /// Notes: Creating a on a random port
    pub fn new() -> MockServer {
        let addr: SocketAddr = ([0, 0, 0, 0], 0).into();
        let mocks: Mocks = Default::default();

        let service = {
            with_sendable(mocks.clone())
                .and(filters::path::full())
                .and(filters::query::raw())
                .and(filters::body::json())
                .and(filters::method::method())
                .and_then(json_body_route)
                .or(with_sendable(mocks.clone())
                    .and(filters::path::full())
                    .and(filters::query::raw())
                    .and(filters::body::bytes())
                    .and(filters::method::method())
                    .and_then(bytes_body_route))
                .or(with_sendable(mocks.clone())
                    .and(filters::path::full())
                    .and(filters::query::raw())
                    .and(filters::method::method())
                    .and_then(no_body_route))
                .or(with_sendable(mocks.clone())
                    .and(filters::path::full())
                    .and(filters::body::json())
                    .and(filters::method::method())
                    .and_then(json_body_route_no_queries))
                .or(with_sendable(mocks.clone())
                    .and(filters::path::full())
                    .and(filters::method::method())
                    .and_then(no_body_route_no_queries))
        };
        let (s, r) = oneshot::channel();

        let (address, server) = warp::serve(service).bind_with_graceful_shutdown(addr, async {
            r.await.unwrap();
        });
        tokio::spawn(server);
        println!("Starting server on {}", address);
        MockServer {
            mocks,
            address,
            kill: Some(s),
        }
    }

    /// Use this to change the behaviour of the server, adding in a replay.
    pub fn with_mock(self, mock: RunMock) -> Self {
        self.mocks.lock().unwrap().push(Arc::new(mock));
        self
    }

    /// Use this to change the behaviour of the server, adding in a replay.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}/{}", self.address, path)
    }

    /// Use this to change the behaviour of the server, filter out in a replay.
    pub fn filter_remove_mock<Filter>(self, filter: Filter) -> Self
    where
        Filter: Fn(&Arc<RunMock>) -> bool,
    {
        {
            let mut filters = self.mocks.lock().unwrap();
            *filters = filters
                .iter()
                .map(|x| x.clone())
                .filter(move |x| filter(x))
                .collect::<Vec<_>>();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use core::time::Duration;
    use std::{fs::remove_file, time::Instant};
    use tokio::{
        sync::{mpsc, oneshot},
        time::timeout,
    };

    use crate::{
        mocks::Gateway,
        mocks::{ClosureMock, FactoryClosure, ReplayMock},
        MockServer,
    };
    use serde_json::{json, Value};
    use tokio::{self, task};

    #[tokio::test]
    async fn capture_and_replay() {
        let file_path = "testingTemp/test.json";
        let client = reqwest::Client::new();
        let body_one: Value = {
            let mock = MockServer::new().with_mock(Gateway::new_replay(
                "",
                "https://jsonplaceholder.typicode.com",
                file_path,
            ));
            let url = format!("http://{}/todos/1", mock.address);

            let res = client.get(&url).send().await.expect("Valid get");
            res.json().await.expect("Serde")
        };

        assert_ne!(body_one, json!(null));
        task::yield_now().await;

        let body_two: Value = {
            let mock = MockServer::new().with_mock(ReplayMock::from_file(file_path));
            let url = format!("http://{}/todos/1", mock.address);
            let res = client.get(&url).send().await.expect("Valid get");
            res.json().await.expect("Serde")
        };

        assert_eq!(body_one, body_two);
        remove_file(file_path).expect("Remove the file for the testing");
    }

    #[tokio::test]
    async fn capture_and_replay_image() {
        let file_path = "testingTemp/test_image.json";
        let client = reqwest::Client::new();
        let body_one: Bytes = {
            let mock = MockServer::new().with_mock(Gateway::new_replay(
                "",
                "https://live.staticflickr.com",
                file_path,
            ));
            let url = format!("http://{}/3903/15218475961_963a4c116e_n.jpg", mock.address);

            let res = client.get(&url).send().await.expect("Valid get");
            res.bytes().await.expect("Serde")
        };

        assert_ne!(body_one, vec![]);
        task::yield_now().await;

        let body_two: Bytes = {
            let mock = MockServer::new().with_mock(ReplayMock::from_file(file_path));
            let url = format!("http://{}/3903/15218475961_963a4c116e_n.jpg", mock.address);
            let res = client.get(&url).send().await.expect("Valid get");
            res.bytes().await.expect("Serde")
        };

        assert_eq!(body_one, body_two);
        remove_file(file_path).expect("Remove the file for the testing");
    }

    #[tokio::test]
    async fn capture_and_replay_text_body() {
        let file_path = "testingTemp/testText.json";
        let client = reqwest::Client::new();
        let body_one: String = {
            let mock = MockServer::new().with_mock(Gateway::new_replay(
                "",
                "https://en.wikipedia.org",
                file_path,
            ));
            let url = format!("http://{}/wiki/Game_replay", mock.address);

            let res = client.get(&url).send().await.expect("Valid get");
            dbg!(res.text().await.unwrap())
        };

        assert_ne!(&body_one, &"");
        task::yield_now().await;

        let body_two: String = {
            let mock = MockServer::new().with_mock(ReplayMock::from_file(file_path));
            let url = format!("http://{}/wiki/Game_replay", mock.address);
            let res = client.get(&url).send().await.expect("Valid get");
            res.text().await.unwrap()
        };

        assert_eq!(body_one, body_two);
        remove_file(file_path).expect("Remove the file for the testing");
    }

    #[tokio::test]
    async fn closure_test() {
        let mock =
            MockServer::new().with_mock(ClosureMock::new(|_req| async { Some(json!("Good")) }));
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
            .with_mock(FactoryClosure::new(move || {
                let send_one = send_one.clone();
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
            .with_mock(FactoryClosure::new(move || {
                let send_two = send_two.clone();
                |req| async move {
                    if &req.path == "/two" {
                        let (send, rec) = oneshot::channel::<Value>();
                        send_two.send(send).await.unwrap();
                        let value = rec.await.unwrap();
                        return Some(value);
                    }
                    None
                }
            }));
        let address = mock.address;
        let body_one = task::spawn(async move {
            let url = format!("http://{}/one", address);

            let client = reqwest::Client::new();

            let body: Value = client
                .get(&url)
                .timeout(Duration::from_millis(100))
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
                .timeout(Duration::from_millis(100))
                .send()
                .await
                .expect("get")
                .json()
                .await
                .expect("json");

            assert_eq!(body, json!("two"));
            Instant::now()
        });

        timeout(Duration::from_millis(100), rec_two.recv())
            .await
            .unwrap()
            .unwrap()
            .send(json!("two"))
            .unwrap();

        timeout(Duration::from_millis(100), rec_one.recv())
            .await
            .unwrap()
            .unwrap()
            .send(json!("one"))
            .unwrap();
        let body_one = body_one.await.unwrap();
        let body_two = body_two.await.unwrap();
        assert!(body_one > body_two);
    }
}
