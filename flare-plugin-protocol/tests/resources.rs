use flare_plugin_protocol::{
    PluginManifest,
    resource::{ResourceRequest, valid_resource_id},
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
    for limit in [0, 101, u32::MAX] {
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
