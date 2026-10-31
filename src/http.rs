use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use crate::model::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

/// Ok for any HTTP status; Err only for transport failure.
pub trait Transport {
    fn send(&self, req: &Request) -> Result<Response>;
}

/// Real transport (ureq 2). A 4xx/5xx comes back as Ok(Response{status, ..}).
pub struct Ureq {
    pub timeout: Duration,
}

impl Transport for Ureq {
    fn send(&self, _req: &Request) -> Result<Response> {
        todo!("U1 impl: Ureq::send")
    }
}

/// Test double: records every request and pops the next queued reply.
pub struct Fake {
    replies: RefCell<VecDeque<Response>>,
    pub calls: RefCell<Vec<Request>>,
}

impl Fake {
    pub fn new(replies: Vec<Response>) -> Fake {
        let _ = replies;
        todo!("U1 impl: Fake::new")
    }
}

impl Transport for Fake {
    fn send(&self, _req: &Request) -> Result<Response> {
        todo!("U1 impl: Fake::send")
    }
}

/// Standard alphabet, '=' padding.
pub fn base64(bytes: &[u8]) -> String {
    let _ = bytes;
    todo!("U1 impl: base64")
}
