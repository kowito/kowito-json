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
        child: Child { id: 1, name: "child".to_string() },
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
