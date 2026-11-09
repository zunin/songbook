mod actions;
mod book;
mod error;
mod model;
mod session;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use book::compose_book;
pub use error::SessionError;
pub use model::{
    BookPartSnapshot, LineSnapshot, MetadataFieldSnapshot, MetadataSnapshot, PreviewRowSnapshot,
    SessionSnapshot, TabSplitBoundsSnapshot, TabSplitPreviewSnapshot, TabSplitRowSnapshot,
};
pub use session::Session;

#[cfg(target_arch = "wasm32")]
pub use wasm::SongbookSession;
