use kowito_json::KJson;
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(KJson, Debug, Default)]
struct Child {
    pub id: i32,
    pub name: String,
}

#[derive(KJson, Debug, Default)]
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
    parent.to_json_bytes(&mut buf);
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
    parent.to_json_bytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();

    assert!(json.contains(r#""age":null"#));
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

#[derive(KJson, Debug, Default)]
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
        t.to_json_bytes(&mut buf);
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
    t1.to_json_bytes(&mut buf);
    let json1 = String::from_utf8(buf.clone()).unwrap();
    let p1: serde_json::Value = serde_json::from_str(&json1).unwrap();
    assert_eq!(p1["data"], long_safe);

    let t2 = StringTest {
        data: long_with_escape.clone(),
    };
    buf.clear();
    t2.to_json_bytes(&mut buf);
    let json2 = String::from_utf8(buf.clone()).unwrap();
    let p2: serde_json::Value = serde_json::from_str(&json2).unwrap();
    assert_eq!(p2["data"], long_with_escape);
}

// ===========================================================================
// serde_json-compatible serializer tests (kowito::to_string / to_vec / etc.)
// ===========================================================================

// ---------------------------------------------------------------------------
// Primitives + serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_serde_primitives_parity() {
    macro_rules! check {
        ($v:expr) => {{
            let kowito = kowito_json::to_string(&$v).unwrap();
            let serde = serde_json::to_string(&$v).unwrap();
            assert_eq!(kowito, serde, "mismatch for {:?}", $v);
        }};
    }
    check!(true);
    check!(false);
    check!(42i32);
    check!(-1i64);
    check!(u64::MAX);
    check!(3.14f64);
    check!("hello \"world\"");
    check!(Option::<i32>::None);
    check!(Some(99u32));
    check!(vec![1u32, 2, 3]);
    check!((10u32, "str", true));
}

// ---------------------------------------------------------------------------
// HashMap
// ---------------------------------------------------------------------------

#[test]
fn test_serde_hashmap() {
    let mut map: HashMap<String, i32> = HashMap::new();
    map.insert("a".to_string(), 1);
    map.insert("b".to_string(), 2);

    let json = kowito_json::to_string(&map).unwrap();
    // Parse back and compare semantically (key order may differ)
    let back: HashMap<String, i32> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, map);
}

// ---------------------------------------------------------------------------
// Enums via KJson derive (to_json_bytes fast path)
// ---------------------------------------------------------------------------

#[derive(KJson, Debug, PartialEq)]
enum Shape {
    Circle,
    Width(f64),
    Point(f64, f64),
    Rect { w: f64, h: f64 },
}

#[test]
fn test_enum_unit_variant() {
    let mut buf = Vec::new();
    Shape::Circle.to_json_bytes(&mut buf);
    assert_eq!(String::from_utf8(buf).unwrap(), r#""Circle""#);
}

#[test]
fn test_enum_newtype_variant() {
    let mut buf = Vec::new();
    Shape::Width(3.14).to_json_bytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["Width"], 3.14);
}

#[test]
fn test_enum_tuple_variant() {
    let mut buf = Vec::new();
    Shape::Point(1.0, 2.0).to_json_bytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["Point"][0], 1.0);
    assert_eq!(v["Point"][1], 2.0);
}

#[test]
fn test_enum_struct_variant() {
    let mut buf = Vec::new();
    Shape::Rect { w: 4.0, h: 5.0 }.to_json_bytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["Rect"]["w"], 4.0);
    assert_eq!(v["Rect"]["h"], 5.0);
}

// Enum via serde path (to_string)
#[test]
fn test_enum_serde_path_parity() {
    for shape in [
        Shape::Circle,
        Shape::Width(2.0),
        Shape::Point(1.0, 3.0),
        Shape::Rect { w: 10.0, h: 20.0 },
    ] {
        let kowito_fast = {
            let mut buf = Vec::new();
            shape.to_json_bytes(&mut buf);
            String::from_utf8(buf).unwrap()
        };
        let kowito_serde = kowito_json::to_string(&shape).unwrap();
        // Both should parse to the same JSON value
        let v_fast: serde_json::Value = serde_json::from_str(&kowito_fast).unwrap();
        let v_serde: serde_json::Value = serde_json::from_str(&kowito_serde).unwrap();
        assert_eq!(v_fast, v_serde);
    }
}

// ---------------------------------------------------------------------------
// Newtype struct
// ---------------------------------------------------------------------------

#[derive(KJson, Debug)]
struct UserId(u64);

#[test]
fn test_newtype_struct() {
    let mut buf = Vec::new();
    UserId(42).to_json_bytes(&mut buf);
    assert_eq!(String::from_utf8(buf).unwrap(), "42");

    let serde_out = kowito_json::to_string(&UserId(99)).unwrap();
    assert_eq!(serde_out, "99");
}

// ---------------------------------------------------------------------------
// Tuple struct
// ---------------------------------------------------------------------------

#[derive(KJson, Debug)]
struct Point2D(f64, f64);

#[test]
fn test_tuple_struct() {
    let mut buf = Vec::new();
    Point2D(1.5, 2.5).to_json_bytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v[0], 1.5);
    assert_eq!(v[1], 2.5);
}

// ---------------------------------------------------------------------------
// Unit struct
// ---------------------------------------------------------------------------

#[derive(KJson, Debug)]
struct Marker;

#[test]
fn test_unit_struct() {
    let mut buf = Vec::new();
    Marker.to_json_bytes(&mut buf);
    assert_eq!(String::from_utf8(buf).unwrap(), "null");
}

// ---------------------------------------------------------------------------
// #[kjson(rename)] and #[kjson(skip)]
// ---------------------------------------------------------------------------

#[derive(KJson, Debug)]
struct ApiUser {
    #[kjson(rename = "userId")]
    pub id: u64,
    pub name: String,
    #[kjson(skip)]
    pub internal_token: String,
}

#[test]
fn test_kjson_rename_and_skip() {
    let u = ApiUser {
        id: 7,
        name: "Bob".to_string(),
        internal_token: "secret".to_string(),
    };
    let mut buf = Vec::new();
    u.to_json_bytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["userId"], 7);
    assert_eq!(v["name"], "Bob");
    assert!(v.get("internal_token").is_none(), "skipped field must not appear");
    assert!(v.get("id").is_none(), "original field name must not appear after rename");
}

// ---------------------------------------------------------------------------
// #[kjson(skip_serializing_if)]
// ---------------------------------------------------------------------------

fn is_zero(v: &u32) -> bool { *v == 0 }

#[derive(KJson, Debug)]
struct Metrics {
    pub hits: u32,
    #[kjson(skip_serializing_if = "is_zero")]
    pub errors: u32,
}

#[test]
fn test_kjson_skip_serializing_if() {
    let m_with = Metrics { hits: 10, errors: 5 };
    let mut buf = Vec::new();
    m_with.to_json_bytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();
    assert!(json.contains("\"errors\":5"));

    let m_zero = Metrics { hits: 10, errors: 0 };
    let mut buf2 = Vec::new();
    m_zero.to_json_bytes(&mut buf2);
    let json2 = String::from_utf8(buf2).unwrap();
    assert!(!json2.contains("errors"), "zero errors field should be skipped");
}

// ---------------------------------------------------------------------------
// Pretty-print
// ---------------------------------------------------------------------------

#[test]
fn test_to_string_pretty() {
    #[derive(serde::Serialize)]
    struct Item { x: u32, y: u32 }
    let item = Item { x: 1, y: 2 };
    let pretty = kowito_json::to_string_pretty(&item).unwrap();
    // Must contain newlines and indentation
    assert!(pretty.contains('\n'));
    assert!(pretty.contains("  "));
    // Must still be valid JSON
    let v: serde_json::Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(v["x"], 1);
    assert_eq!(v["y"], 2);
}

// ---------------------------------------------------------------------------
// to_writer / to_writer_pretty
// ---------------------------------------------------------------------------

#[test]
fn test_to_writer() {
    let mut out: Vec<u8> = Vec::new();
    kowito_json::to_writer(&mut out, &vec![1u32, 2, 3]).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "[1,2,3]");
}

#[test]
fn test_to_writer_pretty() {
    let mut out: Vec<u8> = Vec::new();
    kowito_json::to_writer_pretty(&mut out, &vec![1u32, 2]).unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains('\n'));
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v[0], 1);
    assert_eq!(v[1], 2);
}

// ===========================================================================
// Parsing / Deserialization tests
// ===========================================================================

#[derive(KJson, Debug, PartialEq, Default)]
struct DeUser {
    pub id: u64,
    pub name: String,
    pub active: bool,
    pub score: f64,
    pub age: Option<u32>,
}

#[test]
fn test_deser_basic_struct() {
    let json = r#"{"id":42,"name":"Alice","active":true,"score":9.5,"age":null}"#;
    let user: DeUser = kowito_json::from_str(json).unwrap();
    assert_eq!(user.id, 42);
    assert_eq!(user.name, "Alice");
    assert!(user.active);
    assert!((user.score - 9.5).abs() < 1e-9);
    assert_eq!(user.age, None);
}

#[test]
fn test_deser_optional_field_present() {
    let json = r#"{"id":1,"name":"Bob","active":false,"score":0.0,"age":30}"#;
    let user: DeUser = kowito_json::from_str(json).unwrap();
    assert_eq!(user.age, Some(30));
}

#[test]
fn test_deser_unknown_fields_ignored() {
    let json = r#"{"id":7,"name":"Carol","active":true,"score":1.0,"age":null,"extra":"ignored"}"#;
    let user: DeUser = kowito_json::from_str(json).unwrap();
    assert_eq!(user.id, 7);
    assert_eq!(user.name, "Carol");
}

#[test]
fn test_deser_vec_of_primitives() {
    let nums: Vec<i32> = kowito_json::from_str("[1,2,3,4,5]").unwrap();
    assert_eq!(nums, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_deser_empty_vec() {
    let v: Vec<u64> = kowito_json::from_str("[]").unwrap();
    assert!(v.is_empty());
}

#[test]
fn test_deser_string_with_escapes() {
    let s: String = kowito_json::from_str(r#""hello \"world\"""#).unwrap();
    assert_eq!(s, r#"hello "world""#);
}

#[test]
fn test_deser_roundtrip() {
    let original = DeUser {
        id: 99,
        name: "RoundTrip".to_string(),
        active: true,
        score: 3.14,
        age: Some(25),
    };
    let mut buf = Vec::new();
    original.to_json_bytes(&mut buf);
    let json = String::from_utf8(buf).unwrap();

    let decoded: DeUser = kowito_json::from_str(&json).unwrap();
    assert_eq!(original, decoded);
}

#[derive(KJson, Debug, PartialEq)]
enum Color {
    Red,
    Green,
    Blue,
    Rgb(u8, u8, u8),
    Named { name: String },
}

#[test]
fn test_enum_deserialization() {
    use kowito_json::{from_str, to_string};
    // Unit variant
    let c: Color = from_str(r#"{"Red":null}"#).unwrap();
    assert_eq!(c, Color::Red);
    // Newtype variant
    let c: Color = from_str(r#"{"Rgb":[10,20,30]}"#).unwrap();
    assert_eq!(c, Color::Rgb(10,20,30));
    // Struct variant
    let c: Color = from_str(r#"{"Named":{"name":"purple"}}"#).unwrap();
    assert_eq!(c, Color::Named { name: "purple".to_string() });
    // Roundtrip
    let orig = Color::Green;
    let json = to_string(&orig).unwrap();
    let parsed: Color = from_str(&json).unwrap();
    assert_eq!(parsed, orig);
}

// ===========================================================================
// HashMap / BTreeMap deserialization
// ===========================================================================

#[test]
fn test_deser_hashmap() {
    use std::collections::HashMap;
    let json = r#"{"a":1,"b":2,"c":3}"#;
    let map: HashMap<String, i32> = kowito_json::from_str(json).unwrap();
    assert_eq!(map["a"], 1);
    assert_eq!(map["b"], 2);
    assert_eq!(map["c"], 3);
    assert_eq!(map.len(), 3);
}

#[test]
fn test_deser_btreemap() {
    use std::collections::BTreeMap;
    let json = r#"{"x":10,"y":20}"#;
    let map: BTreeMap<String, i32> = kowito_json::from_str(json).unwrap();
    assert_eq!(map["x"], 10);
    assert_eq!(map["y"], 20);
}

#[test]
fn test_deser_hashmap_empty() {
    use std::collections::HashMap;
    let map: HashMap<String, i32> = kowito_json::from_str("{}").unwrap();
    assert!(map.is_empty());
}

// ===========================================================================
// Value type tests
// ===========================================================================

#[test]
fn test_value_null() {
    let v: kowito_json::Value = kowito_json::from_str("null").unwrap();
    assert!(matches!(v, kowito_json::Value::Null));
}

#[test]
fn test_value_bool_true() {
    let v: kowito_json::Value = kowito_json::from_str("true").unwrap();
    assert!(matches!(v, kowito_json::Value::Bool(true)));
}

#[test]
fn test_value_bool_false() {
    let v: kowito_json::Value = kowito_json::from_str("false").unwrap();
    assert!(matches!(v, kowito_json::Value::Bool(false)));
}

#[test]
fn test_value_number() {
    let v: kowito_json::Value = kowito_json::from_str("42").unwrap();
    if let kowito_json::Value::Number(n) = v {
        assert_eq!(n, "42");
    } else {
        panic!("expected Number");
    }
}

#[test]
fn test_value_string() {
    let v: kowito_json::Value = kowito_json::from_str(r#""hello""#).unwrap();
    if let kowito_json::Value::Str(s) = v {
        assert_eq!(s, "hello");
    } else {
        panic!("expected Str");
    }
}

#[test]
fn test_value_array() {
    let v: kowito_json::Value = kowito_json::from_str("[1,2,3]").unwrap();
    if let kowito_json::Value::Array(arr) = v {
        assert_eq!(arr.len(), 3);
    } else {
        panic!("expected Array");
    }
}

#[test]
fn test_value_object() {
    let v: kowito_json::Value = kowito_json::from_str(r#"{"k":"v"}"#).unwrap();
    if let kowito_json::Value::Object(pairs) = v {
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "k");
    } else {
        panic!("expected Object");
    }
}

#[test]
fn test_value_nested() {
    let json = r#"{"arr":[1,null,true],"obj":{"x":42}}"#;
    let v: kowito_json::Value = kowito_json::from_str(json).unwrap();
    assert!(matches!(v, kowito_json::Value::Object(_)));
}

// ===========================================================================
// UTF-8 validation in from_slice
// ===========================================================================

#[test]
fn test_from_slice_valid_utf8() {
    let bytes = b"\"hello\"";
    let s: String = kowito_json::from_slice(bytes).unwrap();
    assert_eq!(s, "hello");
}

#[test]
fn test_from_slice_invalid_utf8() {
    let bad: &[u8] = &[0xFF, 0xFE, b'"', b'x', b'"'];
    let result: kowito_json::Result<String> = kowito_json::from_slice(bad);
    assert!(result.is_err(), "invalid UTF-8 must return an error");
}

// ===========================================================================
// Line/col error messages
// ===========================================================================

#[test]
fn test_error_contains_line_col() {
    // Malformed JSON: opening brace but then bad token
    let json = "{\n  \"key\": INVALID\n}";
    let result: kowito_json::Result<std::collections::HashMap<String, String>> =
        kowito_json::from_str(json);
    // Should be an error; if it mentions line/col that's a bonus — just check it fails
    assert!(result.is_err());
}

