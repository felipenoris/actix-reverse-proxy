
// Based on https://github.com/DoumanAsh/actix-reverse-proxy

extern crate actix_web;
extern crate futures;

use actix_web::{HttpRequest, HttpResponse, HttpMessage, client};
use actix_web::http::header::{HeaderName, HeaderValue, HeaderMap};
use futures::{Stream, Future};

use std::time::Duration;
use std::net::{SocketAddr, IpAddr};
use std::str::FromStr;

#[cfg(test)]
mod tests;

const X_FORWARDED_FOR_HEADER_NAME_BYTES: &'static [u8] = b"X-Forwarded-For";
static DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct ReverseProxy<'a> {
    forward_url: &'a str,
    timeout: Duration,
}

fn x_forwarded_for_header_name() -> HeaderName {
    HeaderName::from_bytes(X_FORWARDED_FOR_HEADER_NAME_BYTES).unwrap()
}

fn add_client_ip(fwd_header_value: &mut String, client_ip: &str) {
    if !fwd_header_value.is_empty() {
        fwd_header_value.push_str(", ");
    }
    fwd_header_value.push_str(client_ip);
}

fn parse_and_add_client_ip(fwd_header_value: &mut String, client_address: &str) {
    match SocketAddr::from_str(client_address) {
        Ok(client_address) => {
            let client_ip = format!("{}", client_address.ip());
            add_client_ip(fwd_header_value, &client_ip);
        },
        Err(_) => {
            match IpAddr::from_str(client_address) {
                Ok(_) => {
                    add_client_ip(fwd_header_value, client_address);
                },
                Err(e) => println!("Failed parsing client IP for {}: {:?}", client_address, e),
            }
        }
    };
}

// based on https://golang.org/src/net/http/httputil/reverseproxy.go
fn remove_connection_headers(headers: &mut HeaderMap) {
    let mut headers_to_delete: Vec<String> = Vec::new();

    if headers.contains_key("Connection") {
        if let Ok(connection_header_value) = headers["Connection"].to_str() {
            for h in connection_header_value.split(',').map(|s| s.trim()) {
                headers_to_delete.push(String::from(h));
            }
        }
    }

    for h in headers_to_delete {
        // DEBUG
        if headers.contains_key(&h) {
            println!("Removing Connection header `{}`", h);
        }

        headers.remove(h);
    }
}

// based on https://golang.org/src/net/http/httputil/reverseproxy.go
fn disable_user_agent_if_not_set(headers: &mut HeaderMap) {
    let user_agent_header = actix_web::http::header::USER_AGENT;

    if !headers.contains_key(&user_agent_header) {
        headers.insert(&user_agent_header, HeaderValue::from_static(""));
    }
}

// based on https://golang.org/src/net/http/httputil/reverseproxy.go
const HOP_BY_HOP_HEADERS: [&str; 9] = [
        "Connection",
        "Proxy-Connection",
        "Keep-Alive",
        "Proxy-Authenticate",
        "Proxy-Authorization",
        "Te",
        "Trailer",
        "Transfer-Encoding",
        "Upgrade",
];

// based on https://golang.org/src/net/http/httputil/reverseproxy.go
fn remove_request_hop_by_hop_headers(headers: &mut HeaderMap) {
    for h in HOP_BY_HOP_HEADERS.iter() {
        if headers.contains_key(*h) && (headers[*h] == "" || ( *h == "Te" && headers[*h] == "trailers")  ) {
            continue;
        }
        headers.remove(*h);
    }
}

// based on https://golang.org/src/net/http/httputil/reverseproxy.go
fn remove_response_hop_by_hop_headers(headers: &mut HeaderMap) {
    for h in HOP_BY_HOP_HEADERS.iter() {
        headers.remove(*h);
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
        let fwd_header_name = x_forwarded_for_header_name();
        let mut result = String::new();

        for (key, value) in req.headers() {
            if key == fwd_header_name {
                result.push_str(value.to_str().unwrap());
                break;
            }
        }

        // adds client IP address
        // to x-forwarded-for header
        // if it's available
        let client_connection_info = req.connection_info();
        if let Some(client_address) = client_connection_info.remote() {
            parse_and_add_client_ip(&mut result, client_address)
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
        forward_req.set_header(x_forwarded_for_header_name(), self.x_forwarded_for_value(&req));

        let forward_body = req.payload().from_err();
        let mut forward_req = forward_req
                                    .body(actix_web::Body::Streaming(Box::new(forward_body)))
                                    .expect("To create valid forward request");

        remove_connection_headers(forward_req.headers_mut());
        disable_user_agent_if_not_set(forward_req.headers_mut());
        remove_request_hop_by_hop_headers(forward_req.headers_mut());

        // DEBUG
        println!("#### ClientRequest Headers ####");
        for (key, value) in forward_req.headers() {
            println!("[{:?}] = {:?}", key, value);
        }

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
                            back_rsp.header(key.clone(), value.clone());
                        }

                        let back_body = resp.payload().from_err();
                        let mut back_rsp = back_rsp.body(actix_web::Body::Streaming(Box::new(back_body)));
                        remove_connection_headers(back_rsp.headers_mut());
                        remove_response_hop_by_hop_headers(back_rsp.headers_mut());

                        // DEBUG
                        println!("#### Response Headers ####");
                        for (key, value) in back_rsp.headers() {
                            println!("[{:?}] = {:?}", key, value);
                        }

                        back_rsp
                    })
    }
}
