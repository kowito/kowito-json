//! # Example 07 - Manual Serialize Implementation
//!
//! When you need custom field names, conditional fields, flattening, or
//! any non-standard layout, implement `Serialize` manually using the
//! low-level `write_str_escape` and `write_value` helpers.
//!
//! Run with: `cargo run --example 07_manual_serialize`

use kowito_json::serialize::{Serialize, write_str_escape, write_value};

// -----------------------------------------------------------------------
// A struct with a custom key rename
// -----------------------------------------------------------------------
pub struct RenamedFields {
    pub user_id: u64,         // serializes as "userId"
    pub display_name: String, // serializes as "displayName"
}

impl Serialize for RenamedFields {
    fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(b"{\"userId\":");
        write_value(&self.user_id, buf);
        buf.extend_from_slice(b",\"displayName\":");
        write_str_escape(buf, self.display_name.as_bytes());
        buf.push(b'}');
    }
}

// -----------------------------------------------------------------------
// A struct with a computed / derived field
// -----------------------------------------------------------------------
pub struct Circle {
    pub radius: f64,
}

impl Serialize for Circle {
    fn serialize(&self, buf: &mut Vec<u8>) {
        let area = std::f64::consts::PI * self.radius * self.radius;
        let circumference = 2.0 * std::f64::consts::PI * self.radius;

        buf.extend_from_slice(b"{\"radius\":");
        write_value(&self.radius, buf);
        buf.extend_from_slice(b",\"area\":");
        write_value(&area, buf);
        buf.extend_from_slice(b",\"circumference\":");
        write_value(&circumference, buf);
        buf.push(b'}');
    }
}

// -----------------------------------------------------------------------
// A struct with a conditional / optional field (skip null)
// -----------------------------------------------------------------------
pub struct ApiResponse {
    pub status: u16,
    pub data: Option<String>,
    pub error: Option<String>,
}

impl Serialize for ApiResponse {
    fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(b"{\"status\":");
        write_value(&self.status, buf);

        // Only include "data" if Some
        if let Some(ref d) = self.data {
            buf.extend_from_slice(b",\"data\":");
            write_str_escape(buf, d.as_bytes());
        }

        // Only include "error" if Some
        if let Some(ref e) = self.error {
            buf.extend_from_slice(b",\"error\":");
            write_str_escape(buf, e.as_bytes());
        }

        buf.push(b'}');
    }
}

// -----------------------------------------------------------------------
// A newtype wrapper — serialize the inner value transparently
// -----------------------------------------------------------------------
pub struct UserId(pub u64);

impl Serialize for UserId {
    fn serialize(&self, buf: &mut Vec<u8>) {
        write_value(&self.0, buf);
    }
}

// -----------------------------------------------------------------------
// A tagged enum
// -----------------------------------------------------------------------
pub enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}

impl Serialize for Shape {
    fn serialize(&self, buf: &mut Vec<u8>) {
        match self {
            Shape::Circle { radius } => {
                buf.extend_from_slice(b"{\"type\":\"circle\",\"radius\":");
                write_value(radius, buf);
                buf.push(b'}');
            }
            Shape::Rectangle { width, height } => {
                buf.extend_from_slice(b"{\"type\":\"rectangle\",\"width\":");
                write_value(width, buf);
                buf.extend_from_slice(b",\"height\":");
                write_value(height, buf);
                buf.push(b'}');
            }
        }
    }
}

fn print(label: &str, buf: &[u8]) {
    println!("{label:<20} {}", std::str::from_utf8(buf).unwrap());
}

fn main() {
    let mut buf = Vec::new();

    // Renamed fields
    let renamed = RenamedFields {
        user_id: 7,
        display_name: "Kowito".to_string(),
    };
    renamed.serialize(&mut buf);
    print("Renamed fields:", &buf);
    buf.clear();

    // Computed fields
    let circle = Circle { radius: 5.0 };
    circle.serialize(&mut buf);
    print("Circle (computed):", &buf);
    buf.clear();

    // Conditional fields – success response
    let ok = ApiResponse {
        status: 200,
        data: Some("ok".into()),
        error: None,
    };
    ok.serialize(&mut buf);
    print("ApiResponse OK:", &buf);
    buf.clear();

    // Conditional fields – error response
    let err = ApiResponse {
        status: 500,
        data: None,
        error: Some("internal error".into()),
    };
    err.serialize(&mut buf);
    print("ApiResponse Err:", &buf);
    buf.clear();

    // Newtype
    let uid = UserId(42);
    uid.serialize(&mut buf);
    print("UserId newtype:", &buf);
    buf.clear();

    // Tagged enum – circle
    let s1 = Shape::Circle { radius: 3.0 };
    s1.serialize(&mut buf);
    print("Shape::Circle:", &buf);
    buf.clear();

    // Tagged enum – rectangle
    let s2 = Shape::Rectangle {
        width: 10.0,
        height: 4.5,
    };
    s2.serialize(&mut buf);
    print("Shape::Rectangle:", &buf);
    buf.clear();
}
