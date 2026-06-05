# Flare Proto General IM Contract Design

> Date: 2026-06-05
> Scope: `flare-proto` common contract and its immediate downstream consumers.

## Goal

Make `flare-proto` the strict, production-grade, business-neutral contract for a generic IM core that can support any business domain through hooks, capability plugins, custom payloads, and SDK extension points.

The contract must preserve IM invariants: per-conversation monotonic sequence, explicit identity separation, idempotent send and operation acknowledgements, offline-first sync, multi-device convergence, CQRS-friendly read models, and opaque extension slots that do not carry stable core semantics.

## Architectural Direction

`flare-proto` should define only common IM contracts. It is not the place for Social rules, product-specific content catalogs, WebRTC/SFU control details, or business workflows.

The core package remains `flare.common.v1` and keeps:

- Identity and context shapes used by IM services and SDKs.
- Message identity, ordering, status, content envelope, retention state, and push display hints.
- Conversation read models and user-level conversation settings.
- Event, ACK, sync, error, MQ, data, and notification envelopes.
- Extension envelopes for custom message content, custom events, and capability signals.

Business and optional capability contracts move outward:

- Social user, relation, and group rules stay in `flare-social` and `flare-social-proto`.
- RTC/WebRTC/SFU-specific signal payloads stay in `flare-sdk-plugin` / `flare-plugin` and service proto such as `sfu_control.proto`.
- Product-specific message content such as vote, task, schedule, mini program, and announcement becomes app-card or custom content, unless a capability plugin owns a typed payload.

## Industry Alignment

Feishu/Lark alignment:

- Keep message pipeline neutral and use pre-send hooks for business gates.
- Keep directory, profile, relation, and group cursors out of IM message seq.
- Provide capability discovery and typed extension slots rather than bloating core message types.

WeChat alignment:

- Preserve reliable send FSM with separate `client_msg_id`, `server_id`, `seq`, `request_id`, ACK, and sync cursor.
- Keep weak-network optimistic UI and server convergence possible.
- Treat recall, edit, delete, read, retention, reaction, pin, and mark as events, not historical message rewrites.

Telegram alignment:

- Keep per-conversation seq sync and cloud replay.
- Support conversation type differences without hard-coding business directories into IM core.
- Support bots and capabilities through command and event extension surfaces.

## Assumptions

- Existing local prototypes do not require compatibility shims.
- `flare-proto` is allowed to make breaking API changes if downstream crates are updated in the same implementation series.
- `flare-im-core` remains the server-side authority for message seq, storage, sync, and push orchestration.
- `flare-im-core-sdk` remains the client-side authority for reliable sending, local storage, sync, and dispatch behavior.
- Optional RTC and burn-after-read behavior currently exists in downstream code and must be migrated deliberately rather than silently removed.

## Bounded Contexts

### Core Message

Owns:

- `server_id`, `client_msg_id`, `conversation_id`, `channel_id`, `seq`, `sender_id`, `message_type`, encoded `MessageContent`, `MessageStatus`.
- Generic retention and visibility state.
- Opaque extension maps.

Does not own:

- Friend rules, group membership policy, red packets, payments, approval flows, SFU rooms, or media-plane negotiation.

### Content Envelope

Owns stable, generic renderable content:

- Text, image, video, audio, file, location, card/app-card, sticker, emoji, quote, forward, thread, rich text, image group, system, notification, custom, placeholder.

Product-specific structures become:

- `AppCardContent` for generic application cards.
- `CustomContent` for domain-specific payloads.
- Capability-owned typed payloads outside `flare.common.v1`.

### Event Stream

Owns:

- Ordered domain events that mutate message or conversation read models.
- Operation events for recall, edit, delete, read receipt, reaction, pin, mark, retention lifecycle, conversation update, presence, typing, and custom events.

Capability events use a generic capability signal instead of importing capability-specific protos into common.

### Sync

Owns:

- Conversation snapshot.
- Per-conversation message/event increment.
- Event replay plan.
- Cursor update.
- Recovery hints for stale cursors and seq gaps.

Sync must not become a generic command bus for unrelated writes. User conversation settings may stay temporarily for SDK ergonomics, but the implementation plan should evaluate moving settings mutation to an explicit command surface in `flare-grpc-proto`.

### ACK

Owns:

- Send ACK.
- Event ACK.
- Push/window ACK.
- Conversation delivery ACK.
- Explicit read ACK.
- Batch ACK for backpressure and weak-network efficiency.

ACK must keep success/failure and structured error information explicit.

### Capability

Owns:

- `capability_id`, `signal_type`, payload bytes, version, and attributes.
- Routing to plugin implementations.

Specific payloads are owned outside common by `flare-plugin`, `flare-sdk-plugin`, or service-specific proto packages.

## Command, Query, Event, Recovery

Command path:

1. SDK creates local pending message with `client_msg_id`.
2. Gateway or orchestrator receives send command.
3. Core runs hooks and capability checks.
4. Core assigns `server_id`, `seq`, and server timestamp.
5. Core writes message/event to storage and event bus.
6. Core returns `SendAck`.
7. Downstream consumers update read models and push tasks.

Query path:

1. SDK syncs conversation list by cursor.
2. SDK syncs a conversation by `last_seq`.
3. SDK replays critical events using event replay policy.
4. SDK reads history and detail views from read-optimized query models.

Event path:

1. All state-changing operations become typed events.
2. Events are idempotent by `event_id` and ordered by conversation seq where they affect a conversation timeline.
3. Ephemeral events such as typing, presence, and live call hints are explicitly marked as non-history or capability signals.

Recovery path:

1. Client reports last applied seq and cursor.
2. Server detects stale cursor, retention cutoff, or missing event range.
3. Server returns structured recovery hints.
4. Client refetches snapshot, resyncs one conversation, or replays events based on the hint.

## Recommended Design

### 1. Rename Burn Semantics To Generic Retention

Replace business-specific burn terminology with generic retention naming in `message.proto` and `event.proto`.

Recommended common concepts:

- `MessageRetentionPolicy`: retention mode, expire trigger, delay seconds, visibility after expiration.
- `MessageRetentionState`: current lifecycle, first trigger time, expire at, expired at, purged at.
- `MessageRetentionEvent`: scheduled, expired, purged.

Existing burn-after-read maps to:

- trigger: `RETENTION_TRIGGER_AFTER_READ`
- visibility after expiration: `CONTENT_VISIBILITY_HIDDEN_OR_REDACTED`
- lifecycle: scheduled, expired, purged

This keeps Signal/Telegram-style self-destruct messages possible without making the core contract sound like one specific feature.

### 2. Replace RTC-Specific Common Event With Capability Signal

Remove direct `event.proto` dependency on `call_signal.proto`.

Add a generic event payload:

- `CapabilitySignalEvent`
- fields: `capability_id`, `signal_type`, `version`, `payload`, `attributes`

RTC sends:

- `capability_id = "rtc.call"`
- `signal_type = "invite" | "accept" | "hangup" | "ice" | "sfu.join_hints" | ...`
- `payload` is plugin-owned protobuf or JSON.

The `flare-sdk-plugin-call` package keeps typed builders for call signals, but those builders encode plugin-owned payloads into the common capability signal. This preserves typed call ergonomics without binding common IM proto to SFU details.

### 3. Collapse Product-Specific Content Into App Card Or Custom

Keep stable generic types only.

Recommended retained content types:

- Text
- Image
- Video
- Audio
- File
- Location
- Card/AppCard
- Sticker
- Emoji
- Quote
- Forward
- Thread
- RichText
- ImageGroup
- System
- Notification
- Custom
- Placeholder

Move or deprecate in common:

- Vote
- Task
- Schedule
- Announcement
- MiniProgram

These become `AppCardContent` or `CustomContent`. Generated SDKs can still expose convenience builders, but those builders should emit app-card/custom payloads rather than requiring new core enum variants.

### 4. Normalize Sync As Query And Recovery

Keep `Sync` as a data-channel request/response surface for IM sync, not as a mixed command bus.

Recommended shape:

- Snapshot query.
- Conversation list incremental query.
- Conversation detail query.
- Conversation participants query.
- Single and multi-conversation message/event query.
- Event stream ACK.
- Cursor get/update.

Potential command-like sync entries such as `UpdateConversationUserSettingsSync` should be documented as transitional or moved to a dedicated command RPC in `flare-grpc-proto`.

### 5. Keep ACK First-Class And Typed

The current addition of `ReadAck` and correction from `ACK_TYPE_CONVERSTION` to `ACK_TYPE_CONVERSATION` is correct.

Improve the contract by ensuring:

- Every ACK has idempotency identity where needed.
- Send ACK has structured error detail.
- Read ACK is separate from delivery ACK.
- Push/window ACK is tied to `EventEnvelope.window_id`.
- Batch ACK supports multiple conversations and devices.

## Alternatives

### Alternative A: Minimal Patch

Only fix downstream compile drift by updating `flare-im-core/src/message/convert.rs` to current burn fields.

Why not:

- Leaves RTC/SFU in common.
- Leaves product-specific message catalog in core.
- Does not achieve the requested generic IM core standard.

### Alternative B: Keep Common As A Full Product Catalog

Keep burn, call, vote, task, schedule, announcement, and mini program as first-class common types.

Why not:

- Makes `flare-proto` resemble one application suite instead of a generic IM core.
- Forces every SDK/platform to carry every optional business feature.
- Makes plugin and capability boundaries weaker over time.

### Recommended: Strict Core Plus Extension/Capability

This is the best fit for a generic IM platform. Core remains small, stable, and production-grade. Optional capabilities stay typed, but outside common.

## Contract Impact

`flare-proto`:

- Modify `message.proto`, `event.proto`, `message_content.proto`, `data.proto`, `sync.proto`, `ack.proto`, `README.md`, and `IM_PROTO_DESIGN.md`.
- Add retention/capability-neutral naming.
- Remove common import dependency from event to call-specific proto.
- Reserve removed field numbers and enum values.

`flare-grpc-proto`:

- Update re-exports.
- Update message service burn/read commands to retention naming.
- Keep plugin/capability service responsible for capability payload routing.

`flare-im-core`:

- Replace `BurnConfig` / `MessageBurnState` conversion with new retention fields.
- Update storage writer/reader/orchestrator to use retention naming.
- Keep old database column migration only if already in storage; map it internally without leaking old names back into proto.

`flare-im-core-sdk`:

- Update message model and sync policy to retention naming.
- Apply capability signal handling for call plugin events.
- Keep offline-first send/sync behavior unchanged.

`flare-sdk-plugin-call`:

- Own typed call payloads and builder APIs.
- Encode/decode call payloads through `CapabilitySignalEvent`.

`flare-im-core-client-sdk`:

- Update sdk-spec message models.
- Regenerate platform bindings and docs.
- Expose app-card/custom builders for product-specific content.

## Consistency Guarantees

- `seq` remains monotonic per conversation and never rewinds.
- `client_msg_id`, `server_id`, `event_id`, `request_id`, ACK ids, and cursor values remain distinct.
- Retention expiration does not rewrite historical ordering.
- Recall/edit/delete/read/reaction/pin/mark/retention are events, not in-place timeline mutations.
- Conversation read models can be eventually consistent, but recovery hints must tell SDKs when snapshot refetch is required.
- Social cursors and IM seq remain separate.
- Capability payloads do not affect core ordering unless the containing event is explicitly part of the conversation event stream.

## Error Handling

- gRPC surfaces transport/application errors through `tonic::Status` with `ErrorDetail` where available.
- `SendAck` retains structured error details for optimistic UI convergence.
- Hook denial maps to structured error reason and retry advice.
- Sync stale states use `SyncStaleContext` and `SyncRecoveryHint`.
- Capability signal failures should not poison the core sync stream; they return capability-specific errors through the capability service or custom payload response.

## Testing Strategy

Smallest checks:

- `cargo test` in `flare-proto`.
- Focused contract tests for ACK, retention event wire values, and capability signal encoding.

Downstream checks:

- `cargo check` in `flare-grpc-proto`.
- `cargo check -p flare-im-core` or equivalent in the `flare-im-core` workspace.
- Focused tests for message conversion, retention lifecycle, sync policy, and event application.
- SDK contract/codegen verification for `flare-im-core-client-sdk`.

Regression focus:

- Send ACK identity mapping.
- Read ACK versus delivery ACK.
- Retention scheduling and expiration idempotency.
- Critical event replay includes retention events.
- Capability signal does not introduce dependency from common proto to plugin proto.
- Conversation sync still returns per-conversation seq, snapshot, cursor, and recovery hints.

## Bottlenecks And Risks

- Downstream drift already exists: `flare-im-core` currently references removed `BurnConfig` / `MessageBurnState`.
- Generated SDKs may require wide updates if message content enum values change.
- Moving call signals out of common affects `flare-sdk-plugin-call`, `flare-im-core-sdk` listeners, and existing generated client bindings.
- Database schemas may keep burn column names temporarily; implementation should map storage names internally while exposing retention names in protocol.
- Overusing `CustomContent` can become stringly typed. Capability-owned payload schemas and SDK builders should be used for high-value recurring capabilities.

## Implementation Order

1. Stabilize `flare-proto` common contract.
2. Update `flare-grpc-proto` extern paths and re-exports.
3. Update `flare-im-core` conversion and orchestration.
4. Update `flare-im-core-sdk` models, sync policy, and event handling.
5. Update `flare-sdk-plugin-call` to encode call payloads through capability signal.
6. Update sdk-spec and regenerate platform adapters.
7. Run targeted checks from common contract outward.
