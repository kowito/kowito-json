#![feature(portable_simd)]
#![feature(core_intrinsics)]

extern crate self as kowito_json;

pub mod arena;
pub mod scanner;
pub mod serialize;
pub mod string;
pub mod view;

pub use arena::Scratchpad;
pub use string::KString;
pub use view::KView;

pub use kowito_json_derive::Kjson;

/// Example binding utilizing Schema-JIT
#[derive(kowito_json_derive::Kjson, Default, Debug)]
pub struct FastUser {
    pub id: u64,
    pub name: String,
    pub active: bool,
}
