#![feature(portable_simd)]
#![feature(core_intrinsics)]

pub mod arena;
pub mod scanner;
pub mod string;
pub mod view;

pub use arena::Scratchpad;
pub use string::KString;
pub use view::KView;

pub use kowito_json_derive::Kjson;

/// Example binding utilizing Schema-JIT
#[derive(Kjson, Default, Debug)]
pub struct FastUser {
    pub id: u64,
    pub name: String,
    pub active: bool,
}
