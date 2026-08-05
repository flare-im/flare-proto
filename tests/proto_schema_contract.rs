use std::fs;
use std::path::Path;

/// 契约门禁：时间字段用 int64 且不带 `*_ms` 后缀，不用 google.protobuf.Timestamp。
///
/// **`reserved` 不在禁止之列。** 这条规则原先把 `reserved` 一并禁掉，理由是
/// 「greenfield 不需要兼容历史」—— 但 `reserved` 保护的是**将来**：字段删掉后
/// 把号占住，防止后人复用同一个号导致线格式错乱。禁掉它等于要求删除一道
/// 防事故的护栏，方向正好反了（见 event.proto MessageEditEvent 的 reserved 3）。
#[test]
fn proto_contract_uses_int64_instants_and_no_timestamp_well_known_type() {
    let proto_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("proto");
    let mut violations = Vec::new();

    for entry in fs::read_dir(&proto_dir).expect("read proto directory") {
        let entry = entry.expect("read proto entry");
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) != Some("proto") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read proto source");
        for (line_idx, line) in source.lines().enumerate() {
            if line.contains("google.protobuf.Timestamp")
                || line.contains("_at_ms")
                || line.contains("_time_ms")
                || line.contains("timestamp_ms")
            {
                violations.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    line_idx + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "proto contract should use int64 instant fields without *_ms suffixes and no google.protobuf.Timestamp:\n{}",
        violations.join("\n")
    );
}

#[test]
fn common_proto_uses_oneof_as_the_only_top_level_discriminator() {
    let proto_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("proto");
    let mut violations = Vec::new();

    let banned_files = ["call_signal.proto"];
    for file in banned_files {
        if proto_dir.join(file).exists() {
            violations.push(format!(
                "{file}: plugin-specific proto must not live in common"
            ));
        }
    }

    for file in ["ack.proto", "data.proto", "sync.proto"] {
        let path = proto_dir.join(file);
        let source = fs::read_to_string(&path).expect("read proto source");

        for banned in [
            "enum AckType",
            "AckType type =",
            "enum DataKind",
            "DataKind kind =",
            "enum SyncKind",
            "SyncKind kind =",
            "enum SyncSliceItemKind",
            "SyncSliceItemKind kind =",
            "bytes payload = 3;",
        ] {
            if source.contains(banned) {
                violations.push(format!("{}: contains `{}`", path.display(), banned));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "common proto should rely on oneof payloads instead of duplicate kind/type discriminators:\n{}",
        violations.join("\n")
    );
}

#[test]
fn common_proto_uses_consistent_extension_and_result_field_names() {
    let proto_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("proto");
    let mut violations = Vec::new();

    for entry in fs::read_dir(&proto_dir).expect("read proto directory") {
        let entry = entry.expect("read proto entry");
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) != Some("proto") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read proto source");
        for (line_idx, line) in source.lines().enumerate() {
            for banned in [
                "map<string, string> metadata",
                "map<string, string> extra",
                "map<string, string> ext",
                "map<string, string> meta",
                "map<string, string> data",
                "map<string, string> params",
                "map<string, string> tags",
                "map<string, string> source_payload",
            ] {
                if line.contains(banned) {
                    violations.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        line_idx + 1,
                        line.trim()
                    ));
                }
            }

            for banned in [
                "bool success =",
                "int32 error_code =",
                "string error_code =",
                "string error_message =",
                "ErrorDetail error_detail",
            ] {
                if line.contains(banned) {
                    violations.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        line_idx + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "common proto should use attributes/extensions/headers consistently and model success/error contracts as result oneofs:\n{}",
        violations.join("\n")
    );
}
