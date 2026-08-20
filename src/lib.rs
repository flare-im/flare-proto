//! Shared protobuf model types for Flare IM.
//!
//! `flare-proto` exposes the common wire contracts used by Flare services and
//! SDK infrastructure: messages, message content, conversations, sync payloads,
//! metadata, notifications, push envelopes, event-bus envelopes, and data
//! packets.
//!
//! The crate intentionally contains common model types only. gRPC service
//! definitions and tonic-generated clients/servers are published separately by
//! `flare-grpc-proto`.
//!
//! # Main Modules
//!
//! - [`common`] re-exports generated `flare.common.v1` protobuf types.
//! - [`response`] provides helpers for packing `prost_types::Any`.
//! - [`metadata_builder`] provides ergonomic constructors for pagination,
//!   filters, actor context, device context, audit context, sorting, and time
//!   ranges.
//! - [`message_content_ext`] provides encoding and decoding helpers for typed
//!   message content.
//!
//! # Build Behavior
//!
//! The crate uses `prost-build` with a vendored `protoc` binary, so downstream
//! users and docs.rs can build it without installing a system protobuf compiler.

pub mod flare {
    pub mod common {
        pub mod v1 {
            #![allow(clippy::large_enum_variant)]

            include!(concat!(env!("OUT_DIR"), "/flare.common.v1.rs"));
        }
    }
}

pub mod common {
    pub use crate::flare::common::v1::*;
}

pub use common::push_envelope;

/// Convenience builders for `prost_types::Any`.
pub mod response;

/// Convenience builders aligned with `metadata.proto`.
pub mod metadata_builder;

// MessageContent extension methods with a unified encode/decode interface.
pub mod message_content_ext;
pub use message_content_ext::{MessageContentExt, decode_message_content, encode_message_content};

pub use response::pack_any;

// Metadata convenience builders used by business layers.
pub use metadata_builder::{
    actor_service, actor_system, actor_tenant_admin, actor_user, actor_with_attributes,
    actor_with_roles, audit_context, device_context, device_with_priority_critical,
    device_with_priority_high, device_with_priority_low, filter_contains, filter_eq, filter_ge,
    filter_gt, filter_in, filter_le, filter_lt, filter_ne, filter_not_in, pagination,
    pagination_first, pagination_with_more, sort_asc, sort_desc, time_range,
    unix_millis_from_seconds,
};

// Re-export commonly used common model types.
pub use common::{
    AckPayload, AppCardAction, AppCardContent, AudioContent, AudioInfo, AuditContext,
    CapabilityPacket, CardContent, ConflictResolution, ConnectionQuality, ContentVisibility,
    ConversationDetailSync, ConversationDetailSyncRes, ConversationParticipant,
    ConversationParticipantsSync, ConversationParticipantsSyncRes,
    ConversationSummary as ConversationSummaryProto, ConversationSyncSlice,
    ConversationUserSettingsSync, ConversationUserSettingsSyncRes, ConversationVersion,
    ConversationsSync, ConversationsSyncRes, CustomContent, CustomPayload, DataPacket, DeleteType,
    DeviceState as ConversationDeviceState, EventBusEnvelope, FileContent, ForwardContent,
    ForwardItem, ForwardMode, GetSyncCursorSync, GetSyncCursorSyncRes, ImageContent, ImageInfo,
    LocationContent, MarkType, MediaAttachment, Mention, Message, MessageContent,
    MessageReadRecord, MessageRetentionExpiredEvent, MessageRetentionLifecycle,
    MessageRetentionPolicy, MessageRetentionPurgedEvent, MessageRetentionScheduledEvent,
    MessageRetentionState, MessageSource, MessageStatus, MessageTimeline, MessageType, MqEnvelope,
    MqPayloadKind, MultiConversationSync, MultiConversationSyncRes, MultiDeviceCursor,
    NotificationContent, NotificationPayload, OfflinePushInfo, Pagination, PresenceHintPacket,
    PushDelivered, PushEnvelope, PushFailed, PushOptions, PushPayloadKind, PushResult,
    PushTargetType, PushTaskEnvelope, PushTaskPayloadKind, QueryEventsSync, QueryEventsSyncRes,
    ReactionAction, RealtimeControlPacket, RetentionMode, RetentionTrigger, SendAccepted, SendAck,
    SendAckDurability, SingleConversationSync, SingleConversationSyncRes, Sync, SyncRes,
    SyncSkipItem, SyncSliceItem, SyncTombstoneItem, SystemPayload, TextContent, TypingStatePacket,
    UpdateSyncCursorSync, UpdateSyncCursorSyncRes, VideoContent, VideoInfo,
};

/// README 里的代码块当 doctest 编译。
///
/// README 是 crates.io 页面渲染的内容，也是外部使用者实际拿得到的文档；
/// 但 `cargo test` / `cargo clippy` 都不碰它，样例跟着 API 一起腐坏没有任何信号。
/// 本仓的样例本来就是按 doctest 写的（末尾那行 `# Ok::<(), _>(())` 是 rustdoc
/// 的隐藏行），只是从来没被接上。
///
/// 用 `cfg(doctest)` 而不是 `#![doc = include_str!(...)]`：后者会把整份 README
/// （标题、徽章、安装说明）灌进 crate 级文档，与 lib.rs 现有的模块说明重复。
/// 这个写法只在跑 doctest 时存在，不影响公开文档。
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeEn;

#[cfg(doctest)]
#[doc = include_str!("../README.zh-CN.md")]
pub struct ReadmeZh;
