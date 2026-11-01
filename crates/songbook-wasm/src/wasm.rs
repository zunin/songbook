use crate::book;
use crate::error::SessionError;
use crate::session::Session;
use serde::{Deserialize, Serialize};
use songbook_core::book::BookPart;
use wasm_bindgen::prelude::*;

fn js_error(error: SessionError) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|error| JsValue::from_str(&format!("serializing snapshot failed: {error}")))
}

/// JavaScript-facing session wrapper. Construct with raw text, then inspect
/// snapshots, apply actions/keys, read preview rows, and export LaTeX.
#[wasm_bindgen]
pub struct SongbookSession {
    inner: Session,
}

#[wasm_bindgen]
impl SongbookSession {
    #[wasm_bindgen(constructor)]
    pub fn new(raw: &str) -> Result<SongbookSession, JsValue> {
        Session::new(raw)
            .map(|inner| SongbookSession { inner })
            .map_err(js_error)
    }

    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.snapshot())
    }

    #[wasm_bindgen(js_name = previewRows)]
    pub fn preview_rows(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.preview_rows())
    }

    #[wasm_bindgen(js_name = exportTex)]
    pub fn export_tex(&self, standalone: bool) -> Result<String, JsValue> {
        self.inner.export_tex(standalone).map_err(js_error)
    }

    /// The song's book part for `composeBook`: the non-standalone LaTeX body
    /// (identical to `exportTex(false)`) plus the song's distinct chords.
    #[wasm_bindgen(js_name = bookParts)]
    pub fn book_parts(&self) -> Result<JsValue, JsValue> {
        let parts = self.inner.book_parts().map_err(js_error)?;
        to_js(&parts)
    }

    #[wasm_bindgen(js_name = handleAction)]
    pub fn handle_action(&mut self, action: &str) -> Result<JsValue, JsValue> {
        self.inner.handle_action(action).map_err(js_error)?;
        self.snapshot()
    }

    #[wasm_bindgen(js_name = handleKey)]
    pub fn handle_key(&mut self, key: &str) -> Result<JsValue, JsValue> {
        self.inner.handle_key(key).map_err(js_error)?;
        self.snapshot()
    }

    #[wasm_bindgen(js_name = insertChar)]
    pub fn insert_char(&mut self, value: &str) -> Result<JsValue, JsValue> {
        self.inner.insert_char(value).map_err(js_error)?;
        self.snapshot()
    }

    #[wasm_bindgen(js_name = insertText)]
    pub fn insert_text(&mut self, text: &str) -> Result<JsValue, JsValue> {
        self.inner.insert_text(text).map_err(js_error)?;
        self.snapshot()
    }
}

/// One pre-rendered song part as JavaScript sends it to `composeBook`: the
/// body from `bookParts()` plus its distinct chords.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookPartInput {
    body: String,
    chords: Vec<String>,
}

/// JavaScript-facing book ferry (ADR0019): composes pre-rendered song parts
/// into one standalone book LaTeX document, entirely inside core. `parts` is
/// an array of `{ body, chords }` objects exactly as
/// `SongbookSession.bookParts()` returns them. The title index is always
/// included; the `chord_index` flag adds the chord index. `null`/empty titles
/// and authors produce no title block.
#[wasm_bindgen(js_name = composeBook)]
pub fn compose_book(
    title: Option<String>,
    author: Option<String>,
    parts: JsValue,
    chord_index: bool,
) -> Result<String, JsValue> {
    let inputs: Vec<BookPartInput> = serde_wasm_bindgen::from_value(parts)
        .map_err(|error| JsValue::from_str(&format!("invalid book parts: {error}")))?;
    let parts = inputs
        .into_iter()
        .map(|part| BookPart {
            body: part.body,
            chords: part.chords,
        })
        .collect::<Vec<_>>();
    book::compose_book(title.as_deref(), author.as_deref(), &parts, chord_index).map_err(js_error)
}

/// JavaScript-facing song-index conversion (ADR0019): converts one `.sxd`
/// index data file, as LaTeX pass 1 writes it, into the `.sbx` content a
/// later pass typesets. The PDF worker calls this between engine compiles to
/// replace the native `texlua songidx.lua` step inside the browser. The
/// conversion itself lives entirely in `songbook_core::songidx` (the pinned
/// `songidx.lua` port); errors surface as plain strings like every other
/// wasm-boundary failure in this crate.
#[wasm_bindgen]
pub fn convert_sxd_to_sbx(input: &str) -> Result<String, JsValue> {
    songbook_core::songidx::convert_sxd_to_sbx(input)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}
