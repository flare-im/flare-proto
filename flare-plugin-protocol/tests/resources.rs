use flare_plugin_protocol::{
    PluginManifest,
    resource::{
        HostCapability, MAX_MEMBERS_CHECK_USER_IDS, MAX_PARTICIPANTS_PAGE, MAX_RESOURCE_BYTES,
        MAX_RESOURCE_REQUEST_BYTES, MembersCheckView, ParticipantPage, ParticipantView,
        ResourceRequest, valid_resource_id,
    },
};
use serde_json::json;

#[test]
fn host_access_requires_an_explicit_operation_permission_scope() {
    let mut value: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/plugin-ui-manifest.json")).unwrap();
    let original = PluginManifest::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        original
            .capabilities
            .iter()
            .flat_map(|c| &c.operations)
            .all(|o| o.host_capabilities.is_empty())
    );
    value["capabilities"][0]["operations"][0]["hostCapabilities"] = json!(["im.conversation.read"]);
    assert!(PluginManifest::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
    let permission = value["capabilities"][0]["operations"][0]["permissionIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let declared = value["permissions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["id"] == permission)
        .unwrap();
    declared["dataScopes"]
        .as_array_mut()
        .unwrap()
        .push(json!("im.conversation.read"));
    assert!(PluginManifest::from_json(&serde_json::to_vec(&value).unwrap()).is_ok());
    value["capabilities"][0]["operations"][0]["hostCapabilities"] =
        json!(["im.conversation.read", "im.conversation.read"]);
    assert!(PluginManifest::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn callbacks_cannot_choose_identity_conversation_url_or_unbounded_pages() {
    for extra in ["tenantId", "userId", "conversationId", "url"] {
        let input = json!({"capability":"im.conversation.read",extra:"other"});
        assert!(serde_json::from_value::<ResourceRequest>(input).is_err());
    }
    for limit in [0, 501, u32::MAX] {
        let input: ResourceRequest = serde_json::from_value(
            json!({"capability":"im.conversation.participants.read","cursor":"","limit":limit}),
        )
        .unwrap();
        assert!(input.validate().is_err());
    }
    for id in ["", "*", "../tenant", "x\n", "https://host"] {
        assert!(!valid_resource_id(id));
    }
    assert!(
        serde_json::from_value::<ResourceRequest>(json!({"capability":"im.sql.execute"})).is_err()
    );
}

#[test]
fn participant_pages_go_up_to_500_and_still_fit_the_response_bound() {
    for limit in [1, 100, 101, MAX_PARTICIPANTS_PAGE] {
        let input: ResourceRequest = serde_json::from_value(
            json!({"capability":"im.conversation.participants.read","cursor":"42","limit":limit}),
        )
        .unwrap();
        input.validate().unwrap();
    }
    // A full page of the longest user ids stays within the response bound.
    let page = ParticipantPage {
        conversation_id: "c".repeat(128),
        participants: (0..MAX_PARTICIPANTS_PAGE)
            .map(|i| ParticipantView {
                user_id: format!("{i:0>128}"),
                roles: vec!["member".into()],
            })
            .collect(),
        next_cursor: "9".repeat(128),
        has_more: true,
    };
    assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_RESOURCE_BYTES);
}

#[test]
fn members_check_is_a_bounded_read_answered_in_request_order() {
    let capability: HostCapability =
        serde_json::from_value(json!("im.conversation.members.check")).unwrap();
    assert_eq!(capability, HostCapability::ConversationMembersCheck);
    assert_eq!(capability.id(), "im.conversation.members.check");
    assert!(!capability.is_write());
    let request = |ids: serde_json::Value| {
        serde_json::from_value::<ResourceRequest>(
            json!({"capability":"im.conversation.members.check","userIds":ids}),
        )
    };
    let ids: Vec<String> = (0..MAX_MEMBERS_CHECK_USER_IDS)
        .map(|i| format!("{i:0>128}"))
        .collect();
    let full = request(json!(ids)).unwrap();
    full.validate().unwrap();
    assert_eq!(full.capability(), HostCapability::ConversationMembersCheck);
    assert!(serde_json::to_vec(&full).unwrap().len() <= MAX_RESOURCE_REQUEST_BYTES);
    assert_eq!(
        serde_json::to_value(request(json!(["u1", "u2"])).unwrap()).unwrap(),
        json!({"capability":"im.conversation.members.check","userIds":["u1","u2"]})
    );
    let mut too_many = ids.clone();
    too_many.push("u-extra".into());
    for bad in [
        json!([]),
        json!(too_many),
        json!(["u1", "u1"]),
        json!(["u1", "../u2"]),
        json!(["*"]),
    ] {
        assert!(request(bad).unwrap().validate().is_err());
    }
    // No conversation, tenant or other selector is accepted.
    for extra in ["conversationId", "tenantId", "groupId"] {
        assert!(
            serde_json::from_value::<ResourceRequest>(
                json!({"capability":"im.conversation.members.check","userIds":["u1"],extra:"x"})
            )
            .is_err()
        );
    }
    assert!(
        serde_json::from_value::<ResourceRequest>(
            json!({"capability":"im.conversation.members.check"})
        )
        .is_err()
    );
    // Answers: every requested id once, in request order.
    let requested: Vec<String> = ["u1", "u2", "u3", "u4"].map(String::from).into();
    let answer = MembersCheckView::partition(&requested, |id| id == "u1" || id == "u3");
    assert_eq!(
        serde_json::to_value(&answer).unwrap(),
        json!({"members":["u1","u3"],"nonMembers":["u2","u4"]})
    );
    answer.validate_for(&requested).unwrap();
    for (members, non_members) in [
        (json!(["u1"]), json!(["u2", "u4"])),
        (json!(["u1", "u3"]), json!(["u2", "u4", "u5"])),
        (json!(["u3", "u1"]), json!(["u2", "u4"])),
        (json!(["u1", "u3", "u2"]), json!(["u2", "u4"])),
    ] {
        let view: MembersCheckView =
            serde_json::from_value(json!({"members": members, "nonMembers": non_members})).unwrap();
        assert!(view.validate_for(&requested).is_err(), "{view:?}");
    }
    assert!(
        serde_json::from_value::<MembersCheckView>(
            json!({"members": [], "nonMembers": [], "total": 0})
        )
        .is_err()
    );
}

#[test]
fn members_check_needs_an_operation_permission_scope_like_every_read() {
    let mut value: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/plugin-ui-manifest.json")).unwrap();
    value["capabilities"][0]["operations"][0]["hostCapabilities"] =
        json!(["im.conversation.members.check"]);
    assert!(PluginManifest::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
    let permission = value["capabilities"][0]["operations"][0]["permissionIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    value["permissions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["id"] == permission)
        .unwrap()["dataScopes"]
        .as_array_mut()
        .unwrap()
        .push(json!("im.conversation.members.check"));
    let manifest = PluginManifest::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        manifest.capabilities[0].operations[0].host_capabilities,
        [HostCapability::ConversationMembersCheck]
    );
}

#[test]
fn realtime_delivery_is_a_bounded_write_to_named_conversation_members() {
    use flare_plugin_protocol::resource::{
        MAX_REALTIME_PAYLOAD_BYTES, MAX_REALTIME_RECIPIENTS, MAX_REALTIME_TTL_MS, RealtimeDelivery,
        ResourceWriteRequest,
    };
    assert!(HostCapability::RealtimeDeliver.is_write());
    assert_eq!(HostCapability::RealtimeDeliver.id(), "im.realtime.deliver");

    let valid = json!({
        "capability": "im.realtime.deliver",
        "conversationId": "1ADW0CVC4ESXQ0046R",
        "userIds": ["u1", "u2"],
        "packetType": "call.snapshot",
        "payload": {"callId": "c1", "version": 3},
        "ttlMs": 60000,
    });
    let request: ResourceWriteRequest = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(request.capability(), HostCapability::RealtimeDeliver);
    request.validate().unwrap();

    // The payload must be an object, and no identity or URL can ride along.
    for (field, value) in [
        ("payload", json!("text")),
        ("payload", json!([1])),
        ("tenantId", json!("other")),
        ("url", json!("https://host")),
    ] {
        let mut input = valid.clone();
        input[field] = value;
        assert!(serde_json::from_value::<ResourceWriteRequest>(input).is_err());
    }

    let mut fields = valid.clone();
    fields.as_object_mut().unwrap().remove("capability");
    let base: RealtimeDelivery = serde_json::from_value(fields).unwrap();
    let invalid = |change: &dyn Fn(&mut RealtimeDelivery)| {
        let mut delivery = base.clone();
        change(&mut delivery);
        delivery.validate().is_err()
    };
    assert!(invalid(&|d| d.user_ids.clear()));
    assert!(invalid(&|d| d.user_ids = vec!["u1".into(), "u1".into()]));
    assert!(invalid(&|d| d.user_ids = vec!["*".into()]));
    assert!(invalid(&|d| {
        d.user_ids = (0..=MAX_REALTIME_RECIPIENTS)
            .map(|i| format!("u{i}"))
            .collect()
    }));
    assert!(invalid(&|d| d.conversation_id = "../other".into()));
    for packet_type in [
        "",
        "Call.snapshot",
        "1call",
        "call snapshot",
        &"a".repeat(65),
    ] {
        assert!(invalid(&|d| d.packet_type = packet_type.to_owned()));
    }
    assert!(invalid(&|d| d.ttl_ms = 0));
    assert!(invalid(&|d| d.ttl_ms = MAX_REALTIME_TTL_MS + 1));
    assert!(invalid(&|d| {
        d.payload
            .insert("blob".into(), json!("x".repeat(MAX_REALTIME_PAYLOAD_BYTES)));
    }));

    // A full request still fits the resource request budget.
    let mut largest = base.clone();
    largest.user_ids = (0..MAX_REALTIME_RECIPIENTS)
        .map(|i| format!("{i:0>128}"))
        .collect();
    largest.payload.insert(
        "blob".into(),
        json!("x".repeat(MAX_REALTIME_PAYLOAD_BYTES - 64)),
    );
    largest.validate().unwrap();
    let bytes = serde_json::to_vec(&ResourceWriteRequest::RealtimeDeliver(largest)).unwrap();
    assert!(
        bytes.len() <= MAX_RESOURCE_REQUEST_BYTES,
        "{} bytes",
        bytes.len()
    );
}
