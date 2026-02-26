use kowito_json::KView;
use kowito_json::arena::Scratchpad;
use kowito_json::scanner::Scanner;
use kowito_json_derive::Kjson;

#[derive(Debug, Kjson)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
}

fn main() {
    let json_bytes = br#"{"id": 42, "name": "Kowito", "is_active": true}"#;

    // 1. Allocate a scratchpad for the tape (usually kept thread-local)
    let mut scratchpad = Scratchpad::new(1024);
    let tape = scratchpad.get_mut_tape();

    // 2. Scan and find all structural characters instantly with SIMD
    let scanner = Scanner::new(json_bytes);
    scanner.scan(tape);

    // 3. Create a zero-decode view
    let _view = KView::new(json_bytes, tape);

    // 4. Instantly bind to a struct
    // Note: Assuming `Kjson` macro generates `from_kview`. We should test it.
    // let user = User::from_kview(&view);
    // println!("Parsed User: {:?}", user);
}
