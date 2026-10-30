use flare_plugin_protocol::{manifest::*, ui::*};
use serde_json::json;
fn contribution() -> UiContribution {
    serde_json::from_value(json!({"id":"example.page.home","kind":"page","schemaVersion":1,"requiredPermissions":["example.read"],"source":{"operation":"example.read","input":{}},"schema":{"title":"Page","fields":[],"presentation":{"renderer":"account","bindings":{"balance":"/amount","missing":"/absent"},"actions":{"submit":"pay","hidden":"hidden"}},"actions":[{"id":"pay","label":"Pay","target":{"kind":"operation","operation":"example.pay"},"input":{}},{"id":"hidden","label":"Hidden","hiddenWhen":[{"path":"/hide","anyOf":["yes"]}],"target":{"kind":"operation","operation":"example.pay"},"input":{}}]}})).unwrap()
}
#[test]
fn presentation_resolves_only_declared_props_and_visible_actions() {
    let c = contribution();
    c.schema.validate().unwrap();
    let v = c.resolve(
        &json!({"amount":"9007199254740993123456789","private":"secret","hide":"yes"}),
        "en-US",
        "user",
    );
    let p = v.presentation.as_ref().unwrap();
    assert_eq!(p.props.len(), 1);
    assert_eq!(p.props["balance"], "9007199254740993123456789");
    assert_eq!(p.actions.len(), 1);
    assert_eq!(p.actions["submit"], "pay");
    assert!(v.into_read_only().presentation.unwrap().actions.is_empty());
}
#[test]
fn presentation_has_no_code_or_undeclared_authority() {
    let mut c = contribution();
    c.schema.presentation.as_mut().unwrap().renderer = "https://remote/code.js".into();
    assert!(c.schema.validate().is_err());
    c.schema.presentation.as_mut().unwrap().renderer = "account".into();
    c.schema
        .presentation
        .as_mut()
        .unwrap()
        .actions
        .insert("fake".into(), "unlisted".into());
    assert!(c.schema.validate().is_err());
    assert!(
        ResolvedPluginView::unavailable_page(UiUnavailableReason::Disabled)
            .presentation
            .is_none()
    );
}
#[test]
fn excessive_presentation_falls_back_to_declared_document() {
    let v = contribution().resolve(&json!({"amount":"x".repeat(66000)}), "en-US", "user");
    assert!(v.available);
    assert!(v.presentation.is_none());
    assert_eq!(v.actions.len(), 2);
}

#[test]
fn projection_removes_references_to_filtered_actions() {
    let mut c = contribution();
    c.schema.retain_targets(&mut |_| false);
    assert!(c.schema.presentation.as_ref().unwrap().actions.is_empty());
    c.schema.validate().unwrap();
}
