# Flare Proto

> ## ℹ️ 这是通信基础设施，不是开箱即用的 IM 产品
>
> 说在前面，免得你 clone 完才发现登不上去：**开源部分不含账号体系**
> （没有注册登录、好友关系、群角色/审批/禁言、朋友圈）。
>
> 但它自带完整且可插拔的鉴权契约，两条路都在开源侧：
>
> - **`CoreJwtTokenValidator`** —— 本地验 JWT。手签一个 token 就能跑起来做
>   demo / POC，**不需要任何用户体系**。
> - **`HttpHookTokenValidator`** —— 把 token POST 到你自己的接口，
>   **这是接入自有用户体系的入口**。
>
> 业务规则同理：`flare-im-core/crates/flare-im-hooks` 提供 8 个扩展点
> （PreSend / PostSend / Delivery / Recall / MessageRead / MessageReaction /
> ConversationLifecycle / ConversationMember）。
>
> 要上生产，你需要自行实现用户体系并按上述契约接入 —— 与 Sendbird /
> Twilio Conversations 的「自带身份」模型一致，区别是 Flare 可自托管、
> 协议与核心可审计。
>
> 边界详情见 [GOVERNANCE.md](GOVERNANCE.md)。


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
