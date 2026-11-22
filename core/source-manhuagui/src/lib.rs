pub mod decrypt;
pub mod parse;

pub use decrypt::{decrypt, DecryptResult, Sl};
pub use parse::{parse_comic, parse_search};
