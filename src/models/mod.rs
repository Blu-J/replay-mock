use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Method {
    Post,
    Put,
    Get,
    Delete,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub path: String,
    pub method: Method,
    pub body: Value,
}
