use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub path: String,
    pub method: Method,
    pub body: Value,
}
