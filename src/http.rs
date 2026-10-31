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
    fn send(&self, req: &Request) -> Result<Response> {
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();
        let mut r = agent.request(&req.method, &req.url);
        for (k, v) in &req.headers {
            r = r.set(k, v);
        }
        let out = match &req.body {
            Some(b) => r.send_string(b),
            None => r.call(),
        };
        match out {
            Ok(resp) => Ok(Response {
                status: resp.status(),
                body: resp.into_string().map_err(|e| e.to_string())?,
            }),
            Err(ureq::Error::Status(code, resp)) => Ok(Response {
                status: code,
                body: resp.into_string().unwrap_or_default(),
            }),
            Err(e) => Err(format!("transport error on {} {}: {e}", req.method, req.url)),
        }
    }
}

/// Test double: records every request and pops the next queued reply.
pub struct Fake {
    replies: RefCell<VecDeque<Response>>,
    pub calls: RefCell<Vec<Request>>,
}

impl Fake {
    pub fn new(replies: Vec<Response>) -> Fake {
        Fake {
            replies: RefCell::new(replies.into()),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl Transport for Fake {
    fn send(&self, req: &Request) -> Result<Response> {
        self.calls.borrow_mut().push(req.clone());
        self.replies
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| format!("Fake: no reply queued for {} {}", req.method, req.url))
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard alphabet, '=' padding.
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}
