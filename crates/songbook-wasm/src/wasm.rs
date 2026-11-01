use crate::error::SessionError;
use crate::session::Session;
use serde::Serialize;
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
