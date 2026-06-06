use flare_proto::common::ack::Payload as AckPayload;
use flare_proto::common::send_ack::Result as SendAckResult;
use flare_proto::common::{
    Ack, ErrorCode, ErrorDetail, ReadAck, SendAccepted, SendAck, SendAckDurability,
};

#[test]
fn read_ack_is_a_first_class_ack_payload() {
    let ack = Ack {
        ack_id: Some("ack-read-1".to_string()),
        ack_at: None,
        payload: Some(AckPayload::Read(ReadAck {
            conversation_id: "conv-1".to_string(),
            read_seq: 42,
            device_id: Some("device-1".to_string()),
            ack_id: Some("ack-read-1".to_string()),
        })),
    };

    match ack.payload {
        Some(AckPayload::Read(read)) => {
            assert_eq!(read.conversation_id, "conv-1");
            assert_eq!(read.read_seq, 42);
            assert_eq!(read.device_id.as_deref(), Some("device-1"));
            assert_eq!(read.ack_id.as_deref(), Some("ack-read-1"));
        }
        other => panic!("expected typed read ack payload, got {other:?}"),
    }
}

#[test]
fn send_ack_uses_typed_result_payloads() {
    let accepted = SendAck {
        client_msg_id: "client-1".to_string(),
        conversation_id: "conv-1".to_string(),
        ack_id: Some("ack-send-1".to_string()),
        result: Some(SendAckResult::Accepted(SendAccepted {
            server_msg_id: "server-1".to_string(),
            conversation_seq: 7,
            server_time: 1_779_999_001_000,
            durability: SendAckDurability::BrokerAccepted as i32,
        })),
    };

    match accepted.result {
        Some(SendAckResult::Accepted(result)) => {
            assert_eq!(result.server_msg_id, "server-1");
            assert_eq!(result.conversation_seq, 7);
            assert_eq!(result.server_time, 1_779_999_001_000);
            assert_eq!(result.durability(), SendAckDurability::BrokerAccepted);
        }
        other => panic!("expected accepted send ack result, got {other:?}"),
    }

    let rejected = SendAck {
        client_msg_id: "client-2".to_string(),
        conversation_id: "conv-1".to_string(),
        ack_id: Some("ack-send-2".to_string()),
        result: Some(SendAckResult::Error(ErrorDetail {
            code: ErrorCode::PermissionDenied as i32,
            reason: "message_denied".to_string(),
            message: "message rejected by policy".to_string(),
            track: "trace-1".to_string(),
        })),
    };

    match rejected.result {
        Some(SendAckResult::Error(error)) => {
            assert_eq!(error.code, ErrorCode::PermissionDenied as i32);
            assert_eq!(error.reason, "message_denied");
            assert_eq!(error.track, "trace-1");
        }
        other => panic!("expected error send ack result, got {other:?}"),
    }
}
