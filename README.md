
# actix-reverse-proxy

[![License][license-img]](LICENSE)
[![travis][travis-img]][travis-url]

[license-img]: http://img.shields.io/badge/license-MIT-brightgreen.svg?style=flat
[travis-img]: https://img.shields.io/travis/felipenoris/actix-reverse-proxy/master.svg?label=Linux
[travis-url]: https://travis-ci.org/felipenoris/actix-reverse-proxy

This is a simple configurable Reverse Proxy for Actix framework.

Based on https://github.com/DoumanAsh/actix-reverse-proxy.

## Working Example

Create a new project.

```shell
cargo new myproxy
```

Add the following to your `Cargo.toml`.

```toml
[dependencies]
actix-web = "0.7"
futures = "0.1"
actix-reverse-proxy = { git = "https://github.com/felipenoris/actix-reverse-proxy" }
```

Edit `main.rs` with the following. In this example, calls to `http://0.0.0.0:13900/dummy/anything?hey=you`
will be proxied to `http://127.0.0.1:13901/dummy/anything?hey=you`.

```rust
extern crate actix_web;
extern crate futures;
extern crate actix_reverse_proxy;

use actix_web::{server, App, HttpRequest};
use futures::Future;
use actix_reverse_proxy::ReverseProxy;

use std::time::Duration;

const REVERSE_PROXY_BIND_ADDRESS: &'static str = "0.0.0.0:13900";

fn dummy(req: HttpRequest) -> impl Future<Item=actix_web::HttpResponse, Error=actix_web::Error> {
    ReverseProxy::new("http://127.0.0.1:13901")
        .timeout(Duration::from_secs(1))
        .forward(req)
}

fn main() {
    server::new(|| App::new()
            .resource("/dummy/{tail:.*}", |r| r.with_async(dummy))
        )
        .bind(REVERSE_PROXY_BIND_ADDRESS)
        .unwrap()
        .run();
}
```
