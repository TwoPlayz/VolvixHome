//! volvix-core: everything a native Volvix Home app knows that isn't drawing.
//!
//! - [`client::Client`]: the server's JSON API (signs in with a Bearer token).
//! - [`session::SavedSession`]: the server address and sign-in, kept on disk between launches.
//! - [`library::Library`]: artists, albums and songs with the lookups the screens need.
//! - [`search`]: in-library search, matching the website's.
//! - [`queue::Queue`]: the play queue with shuffle and repeat.
//! - [`covers::CoverCache`]: covers kept on disk under their version (they never change).
//! - [`stream::HttpFile`]: a song on the server as a seekable file (range requests).
//!
//! Everything is blocking and thread-safe to call from worker threads; no async runtime needed,
//! which keeps it simple to call from Swift later (UniFFI).

pub mod cast;
pub mod client;
pub mod covers;
pub mod downloads;
pub mod error;
pub mod library;
pub mod members;
pub mod mixes;
pub mod model;
pub mod playlists;
pub mod queue;
pub mod search;
pub mod session;
pub mod stream;
pub mod userdata;

pub use client::Client;
pub use error::{Error, Result};
pub use library::Library;
pub use model::*;
pub use queue::{Queue, Repeat};
