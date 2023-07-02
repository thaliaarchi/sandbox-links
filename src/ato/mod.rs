//! A decoder and encoder for Attempt This Online code share links.
//!
//! Supports schema versions 0 and 1, and is based on the implementation as of
//! commit [b1e7ff3](https://github.com/attempt-this-online/attempt-this-online/blob/b1e7ff39c15afc8194d958b8c9bbc5c3ebcd5730/frontend/lib/urls.ts)
//! (2023-06-30).

mod api;
mod link;
mod state;

pub use api::*;
pub use link::*;
pub use state::*;
