use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Method {
    Post,
    Put,
    Get,
    Delete,
}

impl Method {
    pub fn as_method_string(&self) -> String {
        match self {
            Method::Post => "POST".to_string(),
            Method::Put => "PUT".to_string(),
            Method::Get => "GET".to_string(),
            Method::Delete => "DELETE".to_string(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub path: String,
    pub method: Method,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Replay {
    pub when: Request,
    pub then: Value,
}

impl Replay {
    pub fn matches_request(&self, request: &Request) -> bool {
        self.when.path == request.path
            && self.when.method == request.method
            && assert_json_diff::assert_json_eq_no_panic(&self.when.body, &request.body).is_ok()
    }
}
