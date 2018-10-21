
# actix-reverse-proxy

This is a simple configurable Reverse Proxy for Actix framework based on Based on https://github.com/DoumanAsh/actix-reverse-proxy.

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

Edit `main.rs` with the following. In this example, calls to `http://127.0.0.1:13900/dummy/anything?hey=you`
will be proxied to `http://127.0.0.1:13901/dummy/anything?hey=you`.

```rust
extern crate actix_web;
extern crate futures;
extern crate actix_reverse_proxy;

use actix_web::{server, App, HttpRequest};
use futures::Future;
use actix_reverse_proxy::ReverseProxy;

use std::time::Duration;

const REVERSE_PROXY_IP: &'static str = "127.0.0.1";

fn dummy(req: HttpRequest) -> impl Future<Item=actix_web::HttpResponse, Error=actix_web::Error> {
    let proxy_instance = ReverseProxy::new(REVERSE_PROXY_IP, "http://127.0.0.1:13901")
        .timeout(Duration::from_secs(1));

    proxy_instance.forward(req)
}

fn main() {
    let addr = "127.0.0.1:13900";
    println!("proxy server rodando em  {}", addr);

    server::new(|| App::new()
            .resource("/dummy/{tail:.*}", |r| r.with_async(dummy))
        )
        .bind(addr)
        .unwrap()
        .run();
}
```
