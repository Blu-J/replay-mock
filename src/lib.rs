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

pub mod mocks;
pub mod models;

type RunMock = Box<dyn mocks::RunMock + Send + Sync>;

type Mocks = Arc<Mutex<Vec<RunMock>>>;

pub struct MockServer {
    pub mocks: Mocks,
    pub address: SocketAddr,
    kill: Option<oneshot::Sender<()>>,
}
async fn router(mocks: Mocks, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let path = match req.uri().path_and_query() {
        Some(path_query) => format!("{}", path_query),
        _ => return Ok(not_found()),
    };
    let method = match req.method() {
        &HyperMethod::POST => Method::Post,
        &HyperMethod::PUT => Method::Put,
        &HyperMethod::GET => Method::Get,
        &HyperMethod::DELETE => Method::Delete,
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
    for mock in mocks.lock().await.iter() {
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

    pub async fn mock(&mut self, mock: RunMock) {
        self.mocks.lock().await.push(mock);
    }
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

        assert_eq!(body, json!({}));
    }
}
