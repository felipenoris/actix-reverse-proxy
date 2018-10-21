
// Based on https://github.com/DoumanAsh/actix-reverse-proxy

extern crate actix_web;
extern crate futures;

use actix_web::{HttpRequest, HttpResponse, HttpMessage, client};
use futures::{Stream, Future};

use std::time::Duration;

const X_FORWARDED_HEADER_NAME: &'static str = "x-forwarded-for";
static DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct ReverseProxy<'a> {
    proxy_ip_addr: &'a str,
    forward_url: &'a str,
    timeout: Duration,
}

impl<'a> ReverseProxy<'a> {

    pub fn new(proxy_ip_addr: &'a str, forward_url: &'a str) -> ReverseProxy<'a> {
        ReverseProxy{ proxy_ip_addr, forward_url, timeout: DEFAULT_TIMEOUT }
    }

    pub fn timeout(mut self, duration: Duration) -> ReverseProxy<'a> {
        self.timeout = duration;
        self
    }

    fn get_timeout(&self) -> Duration {
        self.timeout
    }

    fn get_proxy_ip(&self) -> &str {
        self.proxy_ip_addr
    }

    fn get_forward_url(&self) -> &str {
        self.forward_url
    }

    fn x_forwarded_header(&self, req: &HttpRequest) -> String {
        let mut result = String::new();

        for (key, value) in req.headers() {
            if key.as_str() == X_FORWARDED_HEADER_NAME {
                result.push_str(value.to_str().unwrap());
                break;
            }
        }

        // adds proxy server IP address
        // to x-forwarded-for header
        // if it's not already there
        let proxy_ip: &str = self.get_proxy_ip();
        if !result.ends_with(proxy_ip) {
            result.push_str(", ");
            result.push_str(proxy_ip);
        }

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
        forward_req.set_header(X_FORWARDED_HEADER_NAME, self.x_forwarded_header(&req));

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
