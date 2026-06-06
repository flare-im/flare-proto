use flare_proto::common::data_packet::Payload as DataPayload;
use flare_proto::common::event::Payload as EventPayload;
use flare_proto::common::message_content::Content;
use flare_proto::common::push_result::Result as PushResultPayload;
use flare_proto::common::sync_slice_item::Payload as SyncSlicePayload;
use flare_proto::common::{
    CapabilityPacket, ContentVisibility, ConversationsSync, ConversationsSyncRes, DataPacket,
    ErrorCode, ErrorDetail, Event, EventType, Message, MessageContent, MessageRetentionLifecycle,
    MessageRetentionPolicy, MessageRetentionScheduledEvent, MessageRetentionState, MessageType,
    PushDelivered, PushFailed, PushResult, RetentionMode, RetentionTrigger, SyncSliceItem,
    TextContent,
};

#[test]
fn message_uses_typed_content_and_split_sequences() {
    let content = MessageContent {
        content: Some(Content::Text(TextContent {
            text: "hello".to_string(),
            mentions: Vec::new(),
        })),
    };

    let message = Message {
        conversation_id: "conv-1".to_string(),
        conversation_seq: 42,
        message_seq: Some(7),
        message_type: MessageType::Text as i32,
        content: Some(content),
        ..Default::default()
    };

    assert_eq!(message.conversation_seq, 42);
    assert_eq!(message.message_seq, Some(7));

    let encoded = flare_proto::encode_message_content(&message).unwrap();
    assert!(!encoded.is_empty());

    let decoded = flare_proto::decode_message_content(&encoded).unwrap();
    match decoded.content {
        Some(Content::Text(text)) => assert_eq!(text.text, "hello"),
        other => panic!("expected typed text content, got {other:?}"),
    }
}

#[test]
fn retention_event_is_generic_and_replayable() {
    let policy = MessageRetentionPolicy {
        mode: RetentionMode::AfterRead as i32,
        trigger: RetentionTrigger::AfterRead as i32,
        expire_after_seconds: Some(30),
        visibility_after_expiration: ContentVisibility::Redacted as i32,
        ..Default::default()
    };
    let state = MessageRetentionState {
        lifecycle: MessageRetentionLifecycle::Scheduled as i32,
        content_visibility: ContentVisibility::Available as i32,
        ..Default::default()
    };

    let event = Event {
        conversation_id: "conv-1".to_string(),
        conversation_seq: 43,
        r#type: EventType::EventMessageRetentionScheduled as i32,
        payload: Some(EventPayload::RetentionScheduled(
            MessageRetentionScheduledEvent {
                conversation_id: "conv-1".to_string(),
                server_msg_id: "msg-1".to_string(),
                reader_id: Some("user-2".to_string()),
                policy: Some(policy),
                state: Some(state),
                scheduled_at: 1_717_171_717_000,
            },
        )),
        ..Default::default()
    };

    assert_eq!(event.conversation_seq, 43);
    assert!(matches!(
        event.payload,
        Some(EventPayload::RetentionScheduled(_))
    ));
}

#[test]
fn capability_packet_is_non_durable_data_payload() {
    let packet = CapabilityPacket {
        capability_id: "rtc.call".to_string(),
        packet_type: "invite".to_string(),
        version: "v1".to_string(),
        payload: b"plugin-owned".to_vec(),
        correlation_id: Some("call-1".to_string()),
        ..Default::default()
    };

    let data = DataPacket {
        payload: Some(DataPayload::Capability(packet)),
    };

    match data.payload {
        Some(DataPayload::Capability(packet)) => {
            assert_eq!(packet.capability_id, "rtc.call");
            assert_eq!(packet.packet_type, "invite");
        }
        other => panic!("expected capability packet, got {other:?}"),
    }
}

#[test]
fn sync_uses_opaque_cursors_and_typed_slice_payloads() {
    let request = ConversationsSync {
        cursor: "opaque:updated-at-and-id".to_string(),
        limit: 50,
        include_deleted: false,
    };
    assert_eq!(request.cursor, "opaque:updated-at-and-id");

    let response = ConversationsSyncRes {
        next_cursor: "opaque:next".to_string(),
        has_more: true,
        ..Default::default()
    };
    assert_eq!(response.next_cursor, "opaque:next");

    let item = SyncSliceItem {
        conversation_seq: 44,
        created_at: 1_717_171_717_000,
        payload: Some(SyncSlicePayload::Message(Message {
            conversation_id: "conv-1".to_string(),
            conversation_seq: 44,
            message_type: MessageType::Text as i32,
            ..Default::default()
        })),
    };

    match item.payload {
        Some(SyncSlicePayload::Message(message)) => {
            assert_eq!(message.conversation_seq, 44);
            assert_eq!(message.conversation_id, "conv-1");
        }
        other => panic!("expected typed sync message payload, got {other:?}"),
    }
}

#[test]
fn push_result_uses_typed_result_payloads() {
    let delivered = PushResult {
        envelope_id: "env-1".to_string(),
        device_id: "device-1".to_string(),
        user_id: "user-1".to_string(),
        result: Some(PushResultPayload::Delivered(PushDelivered {
            pushed_at: 1_779_999_001_000,
        })),
    };

    match delivered.result {
        Some(PushResultPayload::Delivered(result)) => {
            assert_eq!(result.pushed_at, 1_779_999_001_000);
        }
        other => panic!("expected delivered push result, got {other:?}"),
    }

    let failed = PushResult {
        envelope_id: "env-2".to_string(),
        device_id: "device-1".to_string(),
        user_id: "user-1".to_string(),
        result: Some(PushResultPayload::Failed(PushFailed {
            error: Some(ErrorDetail {
                code: ErrorCode::Unavailable as i32,
                reason: "push_unavailable".to_string(),
                message: "push provider unavailable".to_string(),
                track: "trace-1".to_string(),
            }),
            failed_at: 1_779_999_002_000,
        })),
    };

    match failed.result {
        Some(PushResultPayload::Failed(result)) => {
            let error = result.error.expect("push failure should carry ErrorDetail");
            assert_eq!(error.code, ErrorCode::Unavailable as i32);
            assert_eq!(error.reason, "push_unavailable");
            assert_eq!(result.failed_at, 1_779_999_002_000);
        }
        other => panic!("expected failed push result, got {other:?}"),
    }
}
