extern crate self as kowito_json;

pub mod arena;
pub mod error;
pub mod scanner;
pub mod serde_ser;
pub mod serialize;
pub mod string;
pub mod view;

pub use arena::Scratchpad;
pub use error::{Error, Result};
pub use serde_ser::{
    to_string, to_string_pretty, to_vec, to_vec_pretty, to_writer, to_writer_pretty,
};
pub use string::KString;
pub use view::KView;

pub use kowito_json_derive::KJson;

/// Example binding utilizing Schema-JIT
#[derive(kowito_json_derive::KJson, Default, Debug)]
pub struct FastUser {
    pub id: u64,
    pub name: String,
    pub active: bool,
}
