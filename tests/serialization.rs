use kowito_json::Kjson;
use std::borrow::Cow;

#[derive(Kjson, Debug, Default)]
struct Child {
    pub id: i32,
    pub name: String,
}

#[derive(Kjson, Debug, Default)]
struct Parent {
    pub child: Child,
    pub tags: Vec<String>,
    pub age: Option<u32>,
    pub nickname: Cow<'static, str>,
}

#[test]
fn test_nested_serialization() {
    let parent = Parent {
        child: Child {
            id: 1,
            name: "child".to_string(),
        },
        tags: vec!["tag1".to_string(), "tag2".to_string()],
        age: Some(42),
        nickname: Cow::Borrowed("kowito"),
    };

    let mut buf = Vec::new();
    parent.to_kbytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();

    // Check structural correctness
    assert!(json.contains(r#""child":{"id":1,"name":"child"}"#));
    assert!(json.contains(r#""tags":["tag1","tag2"]"#));
    assert!(json.contains(r#""age":42"#));
    assert!(json.contains(r#""nickname":"kowito""#));

    // Verify it parses back with serde (gold standard)
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

#[test]
fn test_option_null() {
    let parent = Parent {
        child: Child::default(),
        tags: vec![],
        age: None,
        nickname: Cow::Borrowed(""),
    };

    let mut buf = Vec::new();
    parent.to_kbytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();

    assert!(json.contains(r#""age":null"#));
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

#[derive(Kjson, Debug, Default)]
struct StringTest {
    pub data: String,
}

#[test]
fn test_string_escaping_edge_cases() {
    let cases = vec![
        "normal",
        "quote \"",
        "backslash \\",
        "newline \n",
        "tab \t",
        "control \x01\x1f",
        "mixed \" \\ \n \t \x0C",
        "Unicode: 🦀, 你好",
    ];

    for case in cases {
        let t = StringTest {
            data: case.to_string(),
        };
        let mut buf = Vec::new();
        t.to_kbytes(&mut buf);
        let json = String::from_utf8(buf).unwrap();

        // Use serde_json to verify the escaped output
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["data"], case, "Failed for case: {:?}", case);
    }
}

#[test]
fn test_long_string_simd_fast_path() {
    // 64-byte string with no escapes to trigger SIMD loop multiple times
    let long_safe = "this_is_a_very_long_string_that_should_not_require_any_escaping_".to_string();
    // 64-byte string with an escape at the end
    let long_with_escape =
        "this_is_a_very_long_string_that_should_not_require_any_escaping_\"".to_string();

    let t1 = StringTest {
        data: long_safe.clone(),
    };
    let mut buf = Vec::new();
    t1.to_kbytes(&mut buf);
    let json1 = String::from_utf8(buf.clone()).unwrap();
    let p1: serde_json::Value = serde_json::from_str(&json1).unwrap();
    assert_eq!(p1["data"], long_safe);

    let t2 = StringTest {
        data: long_with_escape.clone(),
    };
    buf.clear();
    t2.to_kbytes(&mut buf);
    let json2 = String::from_utf8(buf.clone()).unwrap();
    let p2: serde_json::Value = serde_json::from_str(&json2).unwrap();
    assert_eq!(p2["data"], long_with_escape);
}
