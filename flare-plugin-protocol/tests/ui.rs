use flare_plugin_protocol::{PluginManifest, manifest::ClientPlatform, ui::*};
use serde_json::json;
fn catalog() -> UiCatalog {
    let manifest =
        PluginManifest::from_json(include_bytes!("fixtures/plugin-ui-manifest.json")).unwrap();
    UiCatalog {
        tenant_id: "201".into(),
        version: 1,
        valid_until_ms: 10000,
        plugins: vec![ClientPlugin {
            plugin_id: manifest.plugin_id,
            version: manifest.version,
            manifest_digest: format!("sha256:{}", "a".repeat(64)),
            grant_revision: 1,
            extensions: manifest.client_extensions,
            operations: manifest
                .capabilities
                .iter()
                .flat_map(|c| c.operations.iter().map(|o| format!("{}.{}", c.id, o.id)))
                .collect(),
        }],
    }
}
#[test]
fn all_six_platforms_resolve_identical_history_and_declared_actions() {
    let catalog = catalog();
    let message = PluginMessage {
        plugin_id: "example.poll".into(),
        contribution_id: "example.poll.card".into(),
        schema_version: 1,
        summary: "红包".into(),
        payload: json!({"pollId":"one","question":"<img src=x onerror=alert(1)>","currency":"CNY"}),
    };
    let mut expected = None;
    for platform in [
        ClientPlatform::Web,
        ClientPlatform::Tauri,
        ClientPlatform::Android,
        ClientPlatform::Ios,
        ClientPlatform::Flutter,
        ClientPlatform::Harmonyos,
    ] {
        let view = catalog.resolve_message("201", platform, &message, 1000);
        assert!(view.available);
        assert_eq!(view.fields[0].value, "<img src=x onerror=alert(1)>");
        if let Some(expected) = &expected {
            assert_eq!(&view, expected);
        } else {
            expected = Some(view);
        }
        assert!(matches!(
            catalog
                .action(
                    "201",
                    "example.poll",
                    "example.poll.card",
                    "vote",
                    platform,
                    &Default::default(),
                    true,
                    1000
                )
                .unwrap(),
            UiActionTarget::Operation { .. }
        ));
    }
    for (tenant, now, version) in [("202", 1000, 1), ("201", 10000, 1), ("201", 1000, 99)] {
        let mut invalid = message.clone();
        invalid.schema_version = version;
        let fallback = catalog.resolve_message(tenant, ClientPlatform::Web, &invalid, now);
        assert!(!fallback.available);
        assert!(fallback.actions.is_empty());
        assert!(fallback.fields.is_empty());
    }
    let mut uninstalled = catalog.clone();
    uninstalled.plugins.clear();
    assert!(
        !uninstalled
            .resolve_message("201", ClientPlatform::Web, &message, 1000)
            .available
    );
    assert!(
        catalog
            .action(
                "201",
                "example.poll",
                "example.poll.card",
                "vote",
                ClientPlatform::Web,
                &Default::default(),
                false,
                1000
            )
            .is_err()
    );
    assert!(
        catalog
            .action(
                "201",
                "example.poll",
                "example.poll.card",
                "forged",
                ClientPlatform::Web,
                &Default::default(),
                true,
                1000
            )
            .is_err()
    );
}
#[test]
fn executable_ui_and_undeclared_bridge_targets_are_rejected() {
    for raw in [
        json!({"title":"a","script":"alert(1)"}),
        json!({"title":"a","html":"<iframe>"}),
        json!({"title":"a","actions":[{"id":"a","label":"a","target":{"kind":"url","url":"https://evil"}}]}),
    ] {
        assert!(serde_json::from_value::<UiDocument>(raw).is_err());
    }
    let mut catalog = catalog();
    let plugin = &mut catalog.plugins[0];
    plugin.extensions.contributions[0].schema.actions[0].target = UiActionTarget::Native {
        capability: NativeCapability::Camera,
    };
    assert!(
        catalog
            .action(
                "201",
                "example.poll",
                "example.poll.card",
                "details",
                ClientPlatform::Web,
                &[NativeCapability::Camera].into(),
                true,
                1000
            )
            .is_err()
    );
    catalog.plugins[0]
        .extensions
        .native_capabilities
        .push(NativeCapability::Camera);
    assert!(
        catalog
            .action(
                "201",
                "example.poll",
                "example.poll.card",
                "details",
                ClientPlatform::Web,
                &Default::default(),
                true,
                1000
            )
            .is_err()
    );
    assert!(
        catalog
            .action(
                "201",
                "example.poll",
                "example.poll.card",
                "details",
                ClientPlatform::Web,
                &[NativeCapability::Camera].into(),
                true,
                1000
            )
            .is_ok()
    );
}

#[test]
fn localized_ui_and_input_bindings_do_not_forward_injected_fields() {
    let catalog = catalog();
    let message = PluginMessage {
        plugin_id: "example.poll".into(),
        contribution_id: "example.poll.card".into(),
        schema_version: 1,
        summary: "红包".into(),
        payload: json!({"pollId":"one","tenantId":"202","userId":"victim","amount":"99999"}),
    };
    let view = catalog.resolve_with_context(
        &message,
        UiRenderContext {
            tenant: "201",
            platform: ClientPlatform::Web,
            kind: flare_plugin_protocol::manifest::UiContributionKind::MessageContent,
            locale: "en-US",
            viewer: "",
            now_ms: 1000,
        },
    );
    assert_eq!(view.title, "Poll");
    let claim = view.actions.iter().find(|a| a.id == "vote").unwrap();
    assert_eq!(claim.label, "Vote");
    assert_eq!(
        bind_action_input(claim, &message.payload).unwrap(),
        json!({"pollId":"one"})
    );
    assert!(bind_action_input(claim, &json!({"amount":"99999"})).is_err());
    let wrong_surface = catalog.resolve_with_context(
        &message,
        UiRenderContext {
            tenant: "201",
            platform: ClientPlatform::Web,
            kind: flare_plugin_protocol::manifest::UiContributionKind::Page,
            locale: "en-US",
            viewer: "",
            now_ms: 1000,
        },
    );
    assert!(!wrong_surface.available);
    assert!(wrong_surface.actions.is_empty());
}

fn card_view(
    catalog: &UiCatalog,
    payload: serde_json::Value,
    locale: &str,
    viewer: &str,
) -> ResolvedPluginView {
    let message = PluginMessage {
        plugin_id: "example.poll".into(),
        contribution_id: "example.poll.card".into(),
        schema_version: 1,
        summary: "poll".into(),
        payload,
    };
    catalog.resolve_with_context(
        &message,
        UiRenderContext {
            tenant: "201",
            platform: ClientPlatform::Android,
            kind: flare_plugin_protocol::manifest::UiContributionKind::MessageContent,
            locale,
            viewer,
            now_ms: 1000,
        },
    )
}
/// Status words instead of wire values, rows only for known values, and actions
/// hidden by payload state or viewer; older payloads without the paths keep them.
#[test]
fn value_labels_empty_fields_language_fallback_and_hidden_actions() {
    let mut catalog = catalog();
    let card = catalog.plugins[0]
        .extensions
        .contributions
        .iter_mut()
        .find(|c| c.id == "example.poll.card")
        .unwrap();
    let schema: UiDocument = serde_json::from_value(json!({
        "title": "投票",
        "fields": [
            {"label": "问题", "path": "/question"},
            {"label": "状态", "path": "/status", "values": {"open": "进行中", "closed": "已结束"}}
        ],
        "actions": [
            {"id": "vote", "label": "投票", "target": {"kind": "operation", "operation": "example.poll.vote"},
             "input": {"pollId": "/pollId"},
             "hiddenWhen": [
                 {"path": "/status", "anyOf": ["closed"]},
                 {"path": "/voters", "viewer": true, "negate": true}
             ]}
        ],
        "translations": {"en-US": {
            "title": "Poll", "fields": ["Question", "Status"], "actions": {"vote": "Vote"},
            "values": {"/status": {"open": "Open", "closed": "Closed"}}
        }}
    }))
    .unwrap();
    schema.validate().unwrap();
    card.schema = schema;

    // A row whose value is missing is not rendered; an older payload keeps the action.
    let fresh = card_view(
        &catalog,
        json!({"pollId": "p", "question": "Lunch?"}),
        "zh-CN",
        "u1",
    );
    assert_eq!(fresh.fields.len(), 1);
    assert_eq!(fresh.fields[0].value, "Lunch?");
    assert_eq!(fresh.actions.len(), 1);
    assert!(
        serde_json::to_value(&fresh.actions[0])
            .unwrap()
            .get("hiddenWhen")
            .is_none()
    );

    // Enumerated values read as words, per language, with `en` falling back to `en-US`.
    let open = json!({"pollId": "p", "question": "Lunch?", "status": "open", "voters": ["u1"]});
    let zh = card_view(&catalog, open.clone(), "zh-CN", "u1");
    assert_eq!(zh.fields[1].label, "状态");
    assert_eq!(zh.fields[1].value, "进行中");
    let en = card_view(&catalog, open.clone(), "en", "u1");
    assert_eq!(en.title, "Poll");
    assert_eq!(en.fields[1].label, "Status");
    assert_eq!(en.fields[1].value, "Open");
    assert_eq!(en.actions[0].label, "Vote");
    assert_eq!(
        card_view(&catalog, open.clone(), "en-GB", "u1").fields[1].value,
        "Open"
    );
    // An unknown value is shown as sent.
    let odd = card_view(&catalog, json!({"status": "paused"}), "zh-CN", "u1");
    assert_eq!(odd.fields[0].value, "paused");

    // Hidden once closed, and hidden for a viewer who is not among the voters.
    let closed = json!({"pollId": "p", "status": "closed", "voters": ["u1"]});
    assert!(
        card_view(&catalog, closed, "zh-CN", "u1")
            .actions
            .is_empty()
    );
    assert!(
        card_view(&catalog, open.clone(), "zh-CN", "u2")
            .actions
            .is_empty()
    );
    assert_eq!(card_view(&catalog, open, "zh-CN", "u1").actions.len(), 1);
}
#[test]
fn malformed_value_labels_and_conditions_are_rejected() {
    let base = |extra: serde_json::Value| -> UiDocument {
        let mut doc = json!({
            "title": "t",
            "fields": [{"label": "s", "path": "/status"}],
            "actions": [{"id": "a", "label": "a", "target": {"kind": "operation", "operation": "x.y"}}]
        });
        for (key, value) in extra.as_object().unwrap() {
            let (scope, name) = key.split_once('.').unwrap();
            doc[scope][0][name] = value.clone();
        }
        serde_json::from_value(doc).unwrap()
    };
    // Neither a value list nor the viewer.
    assert!(
        base(json!({"actions.hiddenWhen": [{"path": "/status"}]}))
            .validate()
            .is_err()
    );
    // Both at once.
    assert!(
        base(json!({"actions.hiddenWhen": [{"path": "/status", "anyOf": ["x"], "viewer": true}]}))
            .validate()
            .is_err()
    );
    // Not a payload path.
    assert!(
        base(json!({"actions.hiddenWhen": [{"path": "status", "anyOf": ["x"]}]}))
            .validate()
            .is_err()
    );
    // Empty enumerated value.
    assert!(
        base(json!({"fields.values": {"": "x"}}))
            .validate()
            .is_err()
    );
    assert!(base(json!({"fields.values": {"open": "Open"}, "actions.hiddenWhen": [{"path": "/status", "anyOf": ["closed"]}]})).validate().is_ok());
}
