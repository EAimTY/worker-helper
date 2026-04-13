# worker-helper

[![Version](https://img.shields.io/crates/v/worker-helper.svg?style=flat)](https://crates.io/crates/worker-helper)
[![Documentation](https://img.shields.io/badge/docs-release-brightgreen.svg?style=flat)](https://docs.rs/worker-helper)
[![License](https://img.shields.io/crates/l/worker-helper.svg?style=flat)](https://github.com/EAimTY/worker-helper/blob/master/LICENSE)

Helpers for building and decoding HTTP bodies when working with the
[`worker`](https://docs.rs/worker/latest/worker/) crate.

The crate focuses on a small set of utilities:

- `Fetch<T>` sends an `http::Request<T>` through `worker::Fetch`.
- `BodyExt` adds `text`, `json`, and `yaml` decoding helpers to HTTP bodies
  whose data chunks are `bytes::Bytes`.
- `Json` and `Yaml` turn `serde::Serialize` values into request or response
  bodies when the `json` and `yaml` features are enabled.
- `MapErrorBody` and `MapInfallibleErrorBody` adapt body error types so bodies
  can be reused across APIs with different error requirements.

## Features

- `json` enables `Json` and `BodyExt::json`.
- `yaml` enables `Yaml` and `BodyExt::yaml`.

Enable only the formats you need:

```toml
[dependencies]
worker-helper = { version = "0.3.0", features = ["json"] }
```

## Sending a request

```rust,no_run
use bytes::Bytes;
use http::Request;
use http_body_util::Empty;
use worker_helper::{body::BodyExt, Fetch};

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
use worker_helper::body::{BodyExt, JsonBodyError};

async fn parse_json() -> Result<Message, JsonBodyError<std::convert::Infallible>> {
    let body = Full::new(Bytes::from_static(br#"{"message":"ok"}"#));
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
use worker_helper::body::{Json, MapInfallibleErrorBody};

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

`BodyExt` preserves the underlying body error as `Body(E)` in each method's
error type. Text decoding failures are reported as
`TextBodyError::BadUtf8Encoding`; format decoding failures are reported as
`JsonBodyError::Decode` or `YamlBodyError::Decode` when those features are
enabled.

## License

Licensed under either of:

- Apache License, Version 2.0, in [LICENSE-APACHE](LICENSE-APACHE)
- MIT license, in [LICENSE-MIT](LICENSE-MIT)

at your option.
