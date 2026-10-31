//! Golden results of reading the `kind`-tagged UI enums.
//!
//! Every case is read through every input a host or a client uses: text,
//! bytes, a reader, an owned and a borrowed `serde_json::Value`, and the same
//! value as an array element. The recorded outcome is the re-serialized value
//! or the error category and message, so the file pins the wire output, which
//! inputs are refused, and how.
//!
//! `tests/fixtures/kind-tagged-golden.json` holds the expected outcomes.
//! `FLARE_WRITE_GOLDEN=1 cargo test --test kind_tagged_golden` rewrites it; the
//! diff is then reviewed like any other change.
use flare_plugin_protocol::ui::{ResolvedUiBlock, UiActionTarget, UiBlock};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/kind-tagged-golden.json"
);

/// Inputs every tagged enum is read from, beside its own valid documents.
const SHAPES: &[(&str, &str)] = &[
    ("empty object", r#"{}"#),
    ("no kind", r#"{"text":"x"}"#),
    ("unknown kind", r#"{"kind":"bogus"}"#),
    ("kind in another case", r#"{"kind":"Text"}"#),
    ("kind is a number", r#"{"kind":0}"#),
    ("kind is null", r#"{"kind":null}"#),
    ("kind is a bool", r#"{"kind":true}"#),
    ("kind is an object", r#"{"kind":{}}"#),
    ("kind is an array", r#"{"kind":["text"]}"#),
    ("string", r#""text""#),
    ("integer", r#"1"#),
    ("negative integer", r#"-1"#),
    ("float", r#"1.5"#),
    ("null", r#"null"#),
    ("bool", r#"true"#),
    ("empty array", r#"[]"#),
    ("array with an unknown kind", r#"["bogus"]"#),
    ("array with a number kind", r#"[1]"#),
    ("truncated", r#"{"kind":"text","text":"x""#),
    ("trailing comma", r#"{"kind":"text",}"#),
    ("trailing garbage", r#"{"kind":"note","text":"x"} x"#),
    (
        "syntax error after a type error",
        r#"{"kind":"text","text":5 x}"#,
    ),
    ("eof after a type error", r#"{"kind":"text","text":5,"#),
];

const UI_BLOCK: &[(&str, &str)] = &[
    ("text", r#"{"kind":"text","text":"Hello"}"#),
    (
        "text with every field",
        r#"{"kind":"text","path":"/title","maxLines":3,"style":"title","hiddenWhen":[{"path":"/closed","anyOf":["true"]},{"allOf":[{"path":"/a","exists":true},{"viewer":true,"path":"/u","negate":true}]}]}"#,
    ),
    (
        "kind last",
        r#"{"text":"Hello","style":"secondary","kind":"text"}"#,
    ),
    (
        "kind between fields",
        r#"{"text":"Hello","kind":"text","maxLines":2}"#,
    ),
    ("escaped kind key", r#"{"kind":"text","text":"Hello"}"#),
    ("escaped kind value", r#"{"kind":"text","text":"Hello"}"#),
    (
        "unicode text",
        r#"{"kind":"text","text":"红包 ☃ \"quoted\" \\ \n line"}"#,
    ),
    (
        "fields",
        r#"{"kind":"fields","fields":[{"label":"Amount","path":"/amount","values":{"1":"one","2":"two"}}]}"#,
    ),
    (
        "choices from data",
        r#"{"kind":"choices","optionsPath":"/options","labelPath":"/label","countsPath":"/counts","selectedPath":"/mine","totalPath":"/voters","multiplePath":"/multi","maxSelectionsPath":"/max","showResults":"always","closedWhen":[{"path":"/closed","exists":true}],"action":"vote","inputName":"selection"}"#,
    ),
    (
        "choices declared",
        r#"{"kind":"choices","options":[{"label":"Yes"},{"label":"No"}],"showResults":"closed"}"#,
    ),
    (
        "entries",
        r#"{"kind":"entries","itemsPath":"/lines","textPath":"/text","namePath":"/name","userPath":"/user","userIdsPath":"/ids","numbered":false,"limit":5,"countPath":"/count","emptyText":"None","groupPath":"/g","groupLabelPath":"/gl","groupCountPath":"/gc","groupTotalPath":"/gt","valueLabels":{"path":"/state","values":{"done":{"label":"Done","tone":"success"},"open":{"label":"Open"}}}}"#,
    ),
    (
        "entries numbered",
        r#"{"kind":"entries","itemsPath":"/lines","numbered":true}"#,
    ),
    (
        "people",
        r#"{"kind":"people","groups":[{"label":"Going","usersPath":"/going","countPath":"/n","tone":"success"},{"label":"Maybe"}],"max":5}"#,
    ),
    (
        "status",
        r#"{"kind":"status","label":"State","path":"/state","values":{"open":{"label":"Open","tone":"info"},"closed":{"label":"Closed","tone":"neutral"}}}"#,
    ),
    (
        "progress",
        r#"{"kind":"progress","label":"Raised","valuePath":"/raised","totalPath":"/goal"}"#,
    ),
    (
        "time",
        r#"{"kind":"time","label":"Ends","path":"/endsAt","endPath":"/end","countdown":true}"#,
    ),
    ("divider", r#"{"kind":"divider"}"#),
    (
        "divider hidden",
        r#"{"kind":"divider","hiddenWhen":[{"path":"/x","anyOf":["1"]}]}"#,
    ),
    (
        "note",
        r#"{"kind":"note","text":"Heads up","tone":"warning"}"#,
    ),
    (
        "note with null tone",
        r#"{"kind":"note","path":"/n","tone":null}"#,
    ),
    (
        "rows",
        r#"{"kind":"rows","source":"list","path":"/items","keyPath":"/id","columns":[{"label":"Name","path":"/name"},{"label":"At","path":"/at","format":"time","values":{"a":"b"}}],"cursorPath":"/next","actions":["open"],"emptyText":"Nothing","filters":["status"],"valueLabels":{"path":"/s","values":{"x":{"label":"X","tone":"danger"}}}}"#,
    ),
    ("unknown field", r#"{"kind":"text","text":"x","bogus":1}"#),
    ("two unknown fields", r#"{"kind":"text","zzz":1,"aaa":2}"#),
    (
        "type error then unknown field",
        r#"{"kind":"text","text":5,"zzz":1}"#,
    ),
    (
        "unknown field then type error",
        r#"{"kind":"text","zzz":1,"text":5}"#,
    ),
    (
        "field of another kind",
        r#"{"kind":"note","itemsPath":"/x"}"#,
    ),
    ("missing required field", r#"{"kind":"fields"}"#),
    ("two missing required fields", r#"{"kind":"progress"}"#),
    ("wrong field type", r#"{"kind":"text","text":5}"#),
    ("null for a list", r#"{"kind":"divider","hiddenWhen":null}"#),
    (
        "null for an optional field",
        r#"{"kind":"text","text":null}"#,
    ),
    (
        "float for u8",
        r#"{"kind":"text","text":"x","maxLines":1.0}"#,
    ),
    (
        "u8 out of range",
        r#"{"kind":"text","text":"x","maxLines":300}"#,
    ),
    ("negative u8", r#"{"kind":"text","text":"x","maxLines":-1}"#),
    (
        "huge integer",
        r#"{"kind":"text","text":"x","maxLines":18446744073709551616}"#,
    ),
    (
        "unknown enum value",
        r#"{"kind":"text","text":"x","style":"bold"}"#,
    ),
    (
        "enum value as a map",
        r#"{"kind":"text","text":"x","style":{"title":null}}"#,
    ),
    (
        "enum value as a map with an empty object",
        r#"{"kind":"text","text":"x","style":{"title":{}}}"#,
    ),
    (
        "enum value as a map with an empty array",
        r#"{"kind":"text","text":"x","style":{"title":[]}}"#,
    ),
    (
        "enum value as a map with two keys",
        r#"{"kind":"text","text":"x","style":{"title":null,"body":null}}"#,
    ),
    (
        "enum value as a number",
        r#"{"kind":"text","text":"x","style":0}"#,
    ),
    (
        "duplicate kind",
        r#"{"kind":"text","kind":"text","text":"x"}"#,
    ),
    (
        "duplicate kind with another value",
        r#"{"kind":"text","text":"x","kind":"note"}"#,
    ),
    (
        "duplicate field",
        r#"{"kind":"text","text":"a","text":"b"}"#,
    ),
    (
        "duplicate field in a nested struct",
        r#"{"kind":"text","text":"a","hiddenWhen":[{"path":"/a","path":"/b"}]}"#,
    ),
    (
        "duplicate key in a nested map",
        r#"{"kind":"status","path":"/s","values":{"a":{"label":"A"},"a":{"label":"B"}}}"#,
    ),
    (
        "duplicate field before the kind",
        r#"{"text":"a","text":"b","kind":"text"}"#,
    ),
    (
        "unknown field in a nested struct",
        r#"{"kind":"text","text":"a","hiddenWhen":[{"path":"/a","bogus":1}]}"#,
    ),
    (
        "nested struct with two unknown fields",
        r#"{"kind":"text","text":"a","hiddenWhen":[{"zzz":1,"aaa":2}]}"#,
    ),
    ("array form", r#"["text","Hello"]"#),
    ("array form with only the kind", r#"["divider"]"#),
    (
        "array form with every field",
        r#"["text","Hello",null,2,"title",[]]"#,
    ),
    (
        "array form with an extra element",
        r#"["text","Hello",null,2,"title",[],"extra"]"#,
    ),
    ("array form missing a required field", r#"["fields"]"#),
    ("array form with a wrong type", r#"["text",5]"#),
    (
        "nested struct as an array",
        r#"{"kind":"text","text":"a","hiddenWhen":[["/a",["1"],false,false,[],false]]}"#,
    ),
    (
        "nested struct as an array with an extra element",
        r#"{"kind":"text","text":"a","hiddenWhen":[["/a",["1"],false,false,[],false,"extra"]]}"#,
    ),
    (
        "nested struct as a short array",
        r#"{"kind":"fields","fields":[["Amount"]]}"#,
    ),
];

const RESOLVED_UI_BLOCK: &[(&str, &str)] = &[
    ("text", r#"{"kind":"text","text":"Hello","style":"body"}"#),
    (
        "text with lines",
        r#"{"kind":"text","text":"Hello","maxLines":4,"style":"title"}"#,
    ),
    (
        "kind last",
        r#"{"text":"Hello","style":"body","kind":"text"}"#,
    ),
    (
        "fields",
        r#"{"kind":"fields","fields":[{"label":"Amount","value":"5"},{"label":"To","value":"Ann"}]}"#,
    ),
    (
        "choices",
        r#"{"kind":"choices","options":[{"index":0,"label":"Yes","selected":true,"count":3,"percent":75},{"index":1,"label":"No","selected":false}],"multiple":false,"voted":true,"closed":false,"showResults":true,"total":4,"inputName":"selection","maxSelections":1,"action":{"id":"vote","label":"Vote","target":{"kind":"operation","operation":"x.poll.vote"},"input":{"a":"b"},"hiddenWhen":[{"path":"/closed","exists":true}],"form":["note"],"confirm":"Sure?","style":"primary","placement":"menu","command":{"slot":"vote","mode":"retry","terminalErrorCodes":["closed"]},"continuation":{"page":"result","pluginId":"x.poll","input":{"id":"/id"},"when":{"path":"/ok","exists":true}}}}"#,
    ),
    (
        "choices without an action",
        r#"{"kind":"choices","options":[],"multiple":true,"voted":false,"closed":true,"showResults":false,"inputName":"selection"}"#,
    ),
    (
        "entries",
        r#"{"kind":"entries","items":[{"index":1,"text":"Alice","name":"A","userId":"u1","group":{"key":"0","label":"Yes","count":2,"total":4,"percent":50},"badge":{"text":"Done","tone":"success"}},{"index":2,"text":"Bob"}],"numbered":true,"total":2,"more":false,"emptyText":"None","grouped":true}"#,
    ),
    (
        "people",
        r#"{"kind":"people","groups":[{"label":"Going","userIds":["u1","u2"],"count":2,"tone":"success"},{"label":"Maybe","userIds":[],"count":0}]}"#,
    ),
    (
        "status",
        r#"{"kind":"status","label":"State","text":"Open","tone":"info"}"#,
    ),
    (
        "progress",
        r#"{"kind":"progress","label":"Raised","value":5,"total":10,"percent":50}"#,
    ),
    (
        "time",
        r#"{"kind":"time","label":"Ends","atMs":1700000000000,"endMs":1700000600000,"countdown":true}"#,
    ),
    (
        "time before the epoch",
        r#"{"kind":"time","atMs":-5,"countdown":false}"#,
    ),
    ("divider", r#"{"kind":"divider"}"#),
    (
        "note",
        r#"{"kind":"note","text":"Heads up","tone":"warning"}"#,
    ),
    (
        "rows",
        r#"{"kind":"rows","source":"list","columns":[{"label":"Name","format":"text"},{"label":"At","format":"time"}],"rows":[{"key":"r1","cells":[{"text":"A"},{"text":"t","timeMs":5}],"actions":[{"id":"open","input":{"id":"r1","n":[1,2.5,null,true]}}],"detailInput":{"id":"r1"},"badge":{"text":"New","tone":"info"}}],"nextCursor":"c2","actions":[{"id":"open","label":"Open","target":{"kind":"page","page":"detail","pluginId":"x.other"}},{"id":"pick","label":"Pick","target":{"kind":"native","capability":"selectImage"}}],"emptyText":"Nothing","filters":[]}"#,
    ),
    (
        "rows with a duplicate key in a free-form value",
        r#"{"kind":"rows","source":"list","columns":[],"rows":[{"key":"r1","cells":[],"actions":[{"id":"open","input":{"id":"a","id":"b"}}]}],"actions":[]}"#,
    ),
    ("unknown field", r#"{"kind":"divider","bogus":1}"#),
    (
        "missing required field",
        r#"{"kind":"text","text":"Hello"}"#,
    ),
    (
        "wrong field type",
        r#"{"kind":"progress","value":"5","total":10,"percent":50}"#,
    ),
    (
        "i64 out of range",
        r#"{"kind":"time","atMs":9223372036854775808,"countdown":false}"#,
    ),
    ("duplicate kind", r#"{"kind":"divider","kind":"divider"}"#),
    (
        "duplicate field",
        r#"{"kind":"note","text":"a","text":"b"}"#,
    ),
    (
        "duplicate field in a nested action",
        r#"{"kind":"choices","options":[],"multiple":false,"voted":false,"closed":false,"showResults":false,"inputName":"s","action":{"id":"a","id":"b","label":"L","target":{"kind":"operation","operation":"x.op"}}}"#,
    ),
    (
        "duplicate kind in a nested action target",
        r#"{"kind":"choices","options":[],"multiple":false,"voted":false,"closed":false,"showResults":false,"inputName":"s","action":{"id":"a","label":"L","target":{"kind":"operation","kind":"page","operation":"x.op"}}}"#,
    ),
    (
        "unknown nested action target",
        r#"{"kind":"rows","source":"list","columns":[],"rows":[],"actions":[{"id":"open","label":"Open","target":{"kind":"url","url":"https://x"}}]}"#,
    ),
    (
        "unknown field in a nested action target",
        r#"{"kind":"rows","source":"list","columns":[],"rows":[],"actions":[{"id":"open","label":"Open","target":{"kind":"page","page":"p","operation":"x"}}]}"#,
    ),
    (
        "nested action target as an array",
        r#"{"kind":"rows","source":"list","columns":[],"rows":[],"actions":[{"id":"open","label":"Open","target":["page","p","x.other"]}]}"#,
    ),
    (
        "nested enum value as a map with an empty object",
        r#"{"kind":"status","text":"Open","tone":{"info":{}}}"#,
    ),
    ("array form", r#"["note","Heads up","warning"]"#),
    (
        "array form with an extra element",
        r#"["note","Heads up","warning","extra"]"#,
    ),
    (
        "array form missing a required field",
        r#"["status","Open"]"#,
    ),
];

const UI_ACTION_TARGET: &[(&str, &str)] = &[
    ("operation", r#"{"kind":"operation","operation":"x.op"}"#),
    ("page", r#"{"kind":"page","page":"detail"}"#),
    (
        "page of another plugin",
        r#"{"kind":"page","page":"detail","pluginId":"x.other"}"#,
    ),
    (
        "page with a null plugin",
        r#"{"kind":"page","page":"detail","pluginId":null}"#,
    ),
    ("native", r#"{"kind":"native","capability":"selectImage"}"#),
    ("kind last", r#"{"operation":"x.op","kind":"operation"}"#),
    (
        "unknown field",
        r#"{"kind":"operation","operation":"x.op","page":"p"}"#,
    ),
    (
        "snake case field",
        r#"{"kind":"page","page":"p","plugin_id":"x"}"#,
    ),
    ("missing field", r#"{"kind":"operation"}"#),
    (
        "unknown capability",
        r#"{"kind":"native","capability":"selectimage"}"#,
    ),
    (
        "capability as a map",
        r#"{"kind":"native","capability":{"camera":null}}"#,
    ),
    (
        "capability as a map with an empty object",
        r#"{"kind":"native","capability":{"camera":{}}}"#,
    ),
    (
        "duplicate kind",
        r#"{"kind":"operation","kind":"operation","operation":"x"}"#,
    ),
    (
        "duplicate field",
        r#"{"kind":"operation","operation":"a","operation":"b"}"#,
    ),
    ("array form", r#"["operation","x.op"]"#),
    ("array form of a page", r#"["page","detail","x.other"]"#),
    ("array form missing a field", r#"["page"]"#),
    (
        "array form with an extra element",
        r#"["operation","a","b"]"#,
    ),
];

/// The input forms, in the order [`outcomes`] reads them.
const FORMS: [&str; 7] = [
    "str",
    "slice",
    "reader",
    "array str",
    "value",
    "value ref",
    "array value",
];

fn shown<T: Serialize>(result: Result<T, serde_json::Error>) -> String {
    match result {
        Ok(value) => format!("ok {}", serde_json::to_string(&value).unwrap()),
        Err(error) => format!("err {:?}: {error}", error.classify()),
    }
}

/// What reading `input` as a `T` yields through each of [`FORMS`].
fn outcomes<T: DeserializeOwned + Serialize>(input: &str) -> Vec<String> {
    let value = serde_json::from_str::<Value>(input);
    let from_value = |read: &dyn Fn(Value) -> String| match &value {
        Ok(value) => read(value.clone()),
        Err(error) => format!("not JSON: {error}"),
    };
    vec![
        shown(serde_json::from_str::<T>(input)),
        shown(serde_json::from_slice::<T>(input.as_bytes())),
        shown(serde_json::from_reader::<_, T>(input.as_bytes())),
        shown(serde_json::from_str::<Vec<T>>(&format!("[{input}]"))),
        from_value(&|value| shown(serde_json::from_value::<T>(value))),
        from_value(&|value| shown(T::deserialize(&value))),
        from_value(&|value| shown(serde_json::from_value::<Vec<T>>(Value::Array(vec![value])))),
    ]
}

/// A case as recorded: its input, then each distinct outcome under the forms
/// that yield it.
fn recorded(input: &str, outcomes: &[String]) -> Value {
    let mut grouped: Vec<(Vec<&str>, &String)> = Vec::new();
    for (form, outcome) in FORMS.iter().zip(outcomes) {
        match grouped.iter_mut().find(|(_, seen)| *seen == outcome) {
            Some((forms, _)) => forms.push(form),
            None => grouped.push((vec![form], outcome)),
        }
    }
    let mut out = Map::new();
    out.insert("input".into(), input.into());
    for (forms, outcome) in grouped {
        out.insert(forms.join(", "), outcome.as_str().into());
    }
    out.into()
}

/// The outcome of each form in a recorded case.
fn per_form(case: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (forms, outcome) in case.as_object().unwrap() {
        if forms != "input" {
            for form in forms.split(", ") {
                out.insert(form.to_owned(), outcome.as_str().unwrap().to_owned());
            }
        }
    }
    out
}

fn table<T: DeserializeOwned + Serialize>(cases: &[(&str, &str)]) -> Value {
    let mut out = Map::new();
    for (name, input) in SHAPES.iter().chain(cases) {
        let case = recorded(input, &outcomes::<T>(input));
        assert!(
            out.insert((*name).into(), case).is_none(),
            "duplicate case name {name}"
        );
    }
    out.into()
}

fn actual() -> Value {
    serde_json::json!({
        "UiBlock": table::<UiBlock>(UI_BLOCK),
        "ResolvedUiBlock": table::<ResolvedUiBlock>(RESOLVED_UI_BLOCK),
        "UiActionTarget": table::<UiActionTarget>(UI_ACTION_TARGET),
    })
}

#[test]
fn kind_tagged_enums_read_every_input_as_recorded() {
    let actual = actual();
    if std::env::var_os("FLARE_WRITE_GOLDEN").is_some() {
        let text = serde_json::to_string_pretty(&actual).unwrap() + "\n";
        std::fs::write(FIXTURE, text).unwrap();
        return;
    }
    let expected: Value = serde_json::from_str(&std::fs::read_to_string(FIXTURE).unwrap()).unwrap();
    let mut differences = Vec::new();
    for (ty, cases) in actual.as_object().unwrap() {
        for (name, case) in cases.as_object().unwrap() {
            let want = &expected[ty][name];
            if want.is_null() {
                differences.push(format!("{ty} / {name}: not recorded"));
                continue;
            }
            if want["input"] != case["input"] {
                differences.push(format!("{ty} / {name}: recorded with another input"));
                continue;
            }
            let (want, got) = (per_form(want), per_form(case));
            for form in FORMS {
                if want.get(form) != got.get(form) {
                    differences.push(format!(
                        "{ty} / {name} / {form}:\n  recorded {:?}\n  now      {:?}",
                        want.get(form),
                        got.get(form)
                    ));
                }
            }
        }
    }
    for (ty, cases) in expected.as_object().unwrap() {
        for name in cases.as_object().unwrap().keys() {
            if actual[ty][name].is_null() {
                differences.push(format!("{ty} / {name}: recorded but no longer a case"));
            }
        }
    }
    assert!(
        differences.is_empty(),
        "{} outcome(s) differ from {FIXTURE}:\n{}",
        differences.len(),
        differences.join("\n")
    );
}
