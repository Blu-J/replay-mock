use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// These are the allowed methods as per the standard rest
pub enum Method {
    ///REST Post
    Post,
    ///REST Put
    Put,
    ///REST Get
    Get,
    ///REST Delete
    Delete,
    ///REST Trace
    Trace,
    ///REST HEAD
    Head,
    ///REST Connect
    Connect,
    ///REST patch
    Patch,
    ///REST OPTIONS
    Options,
    ///
    Other,
}

impl Method {
    /// Return the method as a string, for parsing reasons
    pub fn as_method_string(&self) -> String {
        match self {
            Method::Post => "POST".to_string(),
            Method::Put => "PUT".to_string(),
            Method::Get => "GET".to_string(),
            Method::Delete => "DELETE".to_string(),
            Method::Trace => "TRACE".to_string(),
            Method::Head => "HEAD".to_string(),
            Method::Connect => "CONNECT".to_string(),
            Method::Patch => "PATCH".to_string(),
            Method::Options => "OPTIONS".to_string(),
            Method::Other => unimplemented!("Unknown method type"),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// A request is the simplified abstracted structure of a json rest
pub struct Request {
    /// Path in the request
    pub path: String,
    /// queries in the request
    pub queries: Option<String>,
    ///
    pub method: Method,
    ///
    pub body: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Replay is the structure to tie a pattern of if you see this then do that
/// This uses fuzzy typing on the request body.
pub struct Replay {
    /// When a request condition happens
    pub when: Request,
    /// Return this value
    pub then: Value,
}

impl Replay {
    /// We want to know when a Replay matches the request coming in
    pub fn matches_request(&self, request: &Request) -> bool {
        self.when.path == request.path
            && self.when.method == request.method
            && self.when.queries == request.queries
            && assert_json_diff::assert_json_include_no_panic(&request.body, &self.when.body).is_ok()
    }
}
