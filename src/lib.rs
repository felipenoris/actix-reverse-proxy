
// Based on:
//   https://github.com/DoumanAsh/actix-reverse-proxy
//   https://golang.org/src/net/http/httputil/reverseproxy.go

extern crate actix_web;
extern crate futures;

#[macro_use]
extern crate lazy_static;

use actix_web::{HttpRequest, HttpResponse, HttpMessage, client};
use actix_web::http::header::{HeaderName, HeaderMap};
use futures::{Stream, Future};

use std::time::Duration;
use std::net::SocketAddr;

#[cfg(test)]
mod tests;

lazy_static! {
    static ref HEADER_X_FORWARDED_FOR: HeaderName = HeaderName::from_lowercase(b"x-forwarded-for").unwrap();

    static ref HOP_BY_HOP_HEADERS: Vec<HeaderName> = vec![
        HeaderName::from_lowercase(b"connection").unwrap(),
        HeaderName::from_lowercase(b"proxy-connection").unwrap(),
        HeaderName::from_lowercase(b"keep-alive").unwrap(),
        HeaderName::from_lowercase(b"proxy-authenticate").unwrap(),
        HeaderName::from_lowercase(b"proxy-authorization").unwrap(),
        HeaderName::from_lowercase(b"te").unwrap(),
        HeaderName::from_lowercase(b"trailer").unwrap(),
        HeaderName::from_lowercase(b"transfer-encoding").unwrap(),
        HeaderName::from_lowercase(b"upgrade").unwrap(),
    ];

    static ref HEADER_TE: HeaderName = HeaderName::from_lowercase(b"te").unwrap();

    static ref HEADER_CONNECTION: HeaderName = HeaderName::from_lowercase(b"connection").unwrap();
}

static DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct ReverseProxy<'a> {
    forward_url: &'a str,
    timeout: Duration,
}

fn add_client_ip(fwd_header_value: &mut String, client_addr: SocketAddr) {
    if !fwd_header_value.is_empty() {
        fwd_header_value.push_str(", ");
    }

    let client_ip_str = &format!("{}", client_addr.ip());
    fwd_header_value.push_str(client_ip_str);
}

fn remove_connection_headers(headers: &mut HeaderMap) {
    let mut headers_to_delete: Vec<String> = Vec::new();
    let header_connection = &(*HEADER_CONNECTION);

    if headers.contains_key(header_connection) {
        if let Ok(connection_header_value) = headers[header_connection].to_str() {
            for h in connection_header_value.split(',').map(|s| s.trim()) {
                headers_to_delete.push(String::from(h));
            }
        }
    }

    for h in headers_to_delete {
        headers.remove(h);
    }
}

fn remove_request_hop_by_hop_headers(headers: &mut HeaderMap) {
    for h in HOP_BY_HOP_HEADERS.iter() {
        if headers.contains_key(h) && (headers[h] == "" || ( h == *HEADER_TE && headers[h] == "trailers")  ) {
            continue;
        }
        headers.remove(h);
    }
}

impl<'a> ReverseProxy<'a> {

    pub fn new(forward_url: &'a str) -> ReverseProxy<'a> {
        ReverseProxy{ forward_url, timeout: DEFAULT_TIMEOUT }
    }

    pub fn timeout(mut self, duration: Duration) -> ReverseProxy<'a> {
        self.timeout = duration;
        self
    }

    fn x_forwarded_for_value(&self, req: &HttpRequest) -> String {
        let mut result = String::new();

        for (key, value) in req.headers() {
            if key == *HEADER_X_FORWARDED_FOR {
                result.push_str(value.to_str().unwrap());
                break;
            }
        }

        // adds client IP address
        // to x-forwarded-for header
        // if it's available
        if let Some(peer_addr) = req.peer_addr() {
            add_client_ip(&mut result, peer_addr);
        }

        result
    }

    fn forward_uri(&self, req: &HttpRequest) -> String {
        let forward_url: &str = self.forward_url;

        let forward_uri = match req.uri().query() {
            Some(query) => format!("{}{}?{}", forward_url, req.uri().path(), query),
            None => format!("{}{}", forward_url, req.uri().path()),
        };

        forward_uri
    }

    pub fn forward(&self, req: HttpRequest) -> impl Future<Item=actix_web::HttpResponse, Error=actix_web::Error>  {

        let mut forward_req = client::ClientRequest::build_from(&req);
        forward_req.uri(self.forward_uri(&req).as_str());
        forward_req.set_header( &(*HEADER_X_FORWARDED_FOR), self.x_forwarded_for_value(&req));

        let forward_body = req.payload().from_err();
        let mut forward_req = forward_req
                                    .no_default_headers()
                                    .set_header_if_none(actix_web::http::header::USER_AGENT, "")
                                    .body(actix_web::Body::Streaming(Box::new(forward_body)))
                                    .expect("To create valid forward request");

        remove_connection_headers(forward_req.headers_mut());
        remove_request_hop_by_hop_headers(forward_req.headers_mut());

        forward_req.send()
                    .timeout(self.timeout)
                    .map_err(|error| {
                        println!("Error: {}", error);
                        error.into()
                    })
                    .map(|resp| {
                        let mut back_rsp = HttpResponse::build(resp.status());

                        // copy headers
                        for (key, value) in resp.headers() {
                            if !HOP_BY_HOP_HEADERS.contains(key) {
                                back_rsp.header(key.clone(), value.clone());
                            }
                        }

                        let back_body = resp.payload().from_err();
                        let mut back_rsp = back_rsp
                            .no_chunking()
                            .body(actix_web::Body::Streaming(Box::new(back_body)));
                        remove_connection_headers(back_rsp.headers_mut());

                        back_rsp
                    })
    }
}
