
// Based on https://github.com/DoumanAsh/actix-reverse-proxy

extern crate actix_web;
extern crate futures;

use actix_web::{HttpRequest, HttpResponse, HttpMessage, client, http::header::HeaderName};
use futures::{Stream, Future};

use std::time::Duration;

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

impl<'a> ReverseProxy<'a> {

    pub fn new(forward_url: &'a str) -> ReverseProxy<'a> {
        ReverseProxy{ forward_url, timeout: DEFAULT_TIMEOUT }
    }

    pub fn timeout(mut self, duration: Duration) -> ReverseProxy<'a> {
        self.timeout = duration;
        self
    }

    fn get_timeout(&self) -> Duration {
        self.timeout
    }

    fn get_forward_url(&self) -> &str {
        self.forward_url
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
        let client_connection_info = req.connection_info();
        let client_remote_ip = client_connection_info.remote().unwrap();

        if !result.is_empty() {
            result.push_str(", ");
        }
        result.push_str(client_remote_ip);

        result
    }

    fn forward_uri(&self, req: &HttpRequest) -> String {
        let forward_url: &str = self.get_forward_url();

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
        forward_req.set_header(actix_web::http::header::USER_AGENT, "");

        let forward_body = req.payload().from_err();
        let forward_req = forward_req.body(actix_web::Body::Streaming(Box::new(forward_body)));

        forward_req.expect("To create valid forward request")
                    .send()
                    .timeout(self.get_timeout())
                    .map_err(|error| {
                        println!("Error: {}", error);
                        error.into()
                    })
                    .map(|resp| {
                        let mut back_rsp = HttpResponse::build(resp.status());
                        for (key, value) in resp.headers() {
                            back_rsp.header(key.clone(), value.clone());
                        }

                        let back_body = resp.payload().from_err();
                        back_rsp.body(actix_web::Body::Streaming(Box::new(back_body)))
                    })
    }
}
