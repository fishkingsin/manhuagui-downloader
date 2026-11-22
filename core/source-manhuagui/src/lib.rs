pub mod client;
pub mod decrypt;
pub mod list;
pub mod list_source;
pub mod parse;

pub use client::{
    img_url, ComicInFavorite, GetFavoriteResult, ManhuaguiClient, SendWithTimeoutMsg, UserProfile,
};
pub use decrypt::{decrypt, DecryptResult, Sl};
pub use list::{browse_latest_url, parse_list_page};
pub use list_source::{map_eyre_to_source_error, ManhuaguiSource};
pub use parse::{parse_comic, parse_search};


