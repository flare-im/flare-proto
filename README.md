# Flare Proto

[![Crates.io](https://img.shields.io/crates/v/flare-proto.svg)](https://crates.io/crates/flare-proto)
[![Documentation](https://docs.rs/flare-proto/badge.svg)](https://docs.rs/flare-proto)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org/)

`flare-proto` contains the shared protobuf model layer for Flare IM.
It publishes the generated Rust types for common envelopes, messages,
conversation sync payloads, metadata, notifications, data packets, and
event-bus envelopes.

This crate intentionally stays focused on common wire contracts. gRPC service
stubs and tonic clients live in `flare-grpc-proto`.

API documentation: [docs.rs/flare-proto](https://docs.rs/flare-proto)

## Installation

```toml
[dependencies]
flare-proto = "1.0.1"
```

Optional feature flags are reserved for client and server integrations:

```toml
flare-proto = { version = "1.0.1", features = ["client"] }
flare-proto = { version = "1.0.1", features = ["server"] }
```

## What Is Included

- Common message and content models.
- Conversation sync request and response payloads.
- Event, topic, and MQ envelope models.
- Metadata helpers for pagination, filters, actors, devices, audit context,
  and time ranges.
- Push envelope and delivery result models.
- Convenience helpers for packing `prost_types::Any`.

## Quick Start

```rust
use flare_proto::{
    MessageContent,
    MessageContentExt,
    TextContent,
    encode_message_content,
    pagination_first,
};

let text = TextContent {
    text: "hello flare".to_string(),
    ..Default::default()
};

let content = MessageContent::from_text(text);
let encoded = encode_message_content(&content)?;
let page = pagination_first(20);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Build Behavior

The package uses `prost-build` and a vendored `protoc` binary during builds, so
users do not need to install a system protobuf compiler just to consume the
crate. The `.proto` files remain the source of truth and are included in the
published package.

## Related Crates

| Crate | Purpose |
|-------|---------|
| `flare-proto` | Common protobuf model types and helpers. |
| `flare-grpc-proto` | gRPC service definitions and tonic-generated stubs. |
| `flare-server-core` | Server-side runtime, transport, messaging, auth, and telemetry infrastructure. |

## License

Licensed under the [MIT License](LICENSE).
