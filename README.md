# worker-helper

[![Version](https://img.shields.io/crates/v/worker-helper.svg?style=flat)](https://crates.io/crates/worker-helper)
[![Documentation](https://img.shields.io/badge/docs-release-brightgreen.svg?style=flat)](https://docs.rs/worker-helper)
[![License](https://img.shields.io/crates/l/worker-helper.svg?style=flat)](https://github.com/EAimTY/worker-helper/blob/master/LICENSE)

Helpers for building and decoding HTTP bodies when working with the
[`worker`](https://docs.rs/worker/latest/worker/) crate.

The crate focuses on a small set of utilities:

- `Fetch<T>` sends an `http::Request<T>` through `worker::Fetch`.
- `Body<E>` wraps a response body and adds `text`, `json`, and `yaml`
  decoding helpers.
- `Json` and `Yaml` turn `serde::Serialize` values into request or response
  bodies when the `json` and `yaml` features are enabled.
- `MapErrorBody` and `MapInfallibleErrorBody` adapt body error types so bodies
  can be reused across APIs with different error requirements.

## Features

- `json` enables `Json`, `Body::json`, and
  `ReceiveBodyError::InvalidJson`.
- `yaml` enables `Yaml`, `Body::yaml`, and
  `ReceiveBodyError::InvalidYaml`.

Enable only the formats you need:

```toml
[dependencies]
worker-helper = { version = "0.1.0", features = ["json"] }
```

## Sending a request

```rust,no_run
use bytes::Bytes;
use http::Request;
use http_body_util::Empty;
use worker_helper::Fetch;

async fn fetch_text() -> Result<String, Box<dyn std::error::Error>> {
    let request = Request::get("https://example.com")
        .body(Empty::<Bytes>::new())?;

    let response = Fetch(request).send().await?;
    let body = response.into_body();

    Ok(body.text().await?)
}
```

## Decoding a streamed body

```rust
# #[cfg(feature = "json")]
# {
use bytes::Bytes;
use http_body_util::Full;
use worker_helper::Body;

async fn parse_json() -> Result<Message, worker_helper::ReceiveBodyError<std::convert::Infallible>> {
    let body = Body::new(Full::new(Bytes::from_static(br#"{"message":"ok"}"#)));
    body.json().await
}

#[derive(serde::Deserialize, PartialEq, Eq, Debug)]
struct Message {
    message: String,
}
# }
```

## Building a JSON response body

```rust
# #[cfg(feature = "json")]
# {
use http::Response;
use worker_helper::{Json, MapInfallibleErrorBody};

#[derive(serde::Serialize)]
struct Payload {
    ok: bool,
}

fn response() -> Response<MapInfallibleErrorBody<Json, worker::Error>> {
    Response::builder()
        .header("content-type", "application/json")
        .body(MapInfallibleErrorBody::new(Json::new(Payload { ok: true })))
        .expect("valid response")
}
# }
```

## Error handling

`Body<E>` preserves the underlying body error as
`ReceiveBodyError::Receive(E)`. Parsing failures are reported as
`ReceiveBodyError::BadUtf8Encoding` and, when enabled,
`ReceiveBodyError::InvalidJson` or `ReceiveBodyError::InvalidYaml`.

## License

Licensed under either of:

- Apache License, Version 2.0, in [LICENSE-APACHE](LICENSE-APACHE)
- MIT license, in [LICENSE-MIT](LICENSE-MIT)

at your option.
