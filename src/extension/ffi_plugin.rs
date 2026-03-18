use std::ffi::{CStr, CString};
use std::path::Path;

use crate::editor::highlight::{Token, TokenKind};
use crate::plugin::{Plugin, PluginContext, PluginResponse};

/// A language plugin loaded from a compiled `.so`/`.dll` extension.
///
/// Wraps the five C FFI functions exported by each language module:
/// `language_id`, `file_extensions`, `tokenize_line_ffi`, `hover_info_ffi`, `free_string`.
pub struct FfiLangPlugin {
    // Must be kept alive — dropping it would unload the library.
    _lib: libloading::Library,
    language_id: String,
    extensions: Vec<String>,
    lsp_server: Option<String>,
    lsp_args: Vec<String>,
    tokenize_fn: Option<unsafe extern "C" fn(*const std::ffi::c_char) -> *mut std::ffi::c_char>,
    free_fn: Option<unsafe extern "C" fn(*mut std::ffi::c_char)>,
    hover_fn: Option<
        unsafe extern "C" fn(
            *const std::ffi::c_char,
            *const std::ffi::c_char,
        ) -> *mut std::ffi::c_char,
    >,
}

// Safety: all raw fn pointers are Send + Sync as long as the underlying C functions are
// thread-safe, which they are by convention for these pure-computation FFI modules.
unsafe impl Send for FfiLangPlugin {}
unsafe impl Sync for FfiLangPlugin {}

impl FfiLangPlugin {
    /// Load a language extension from `lib_path`.
    /// `lsp_server` / `lsp_args` come from the extension's `manifest.toml`.
    pub fn load(
        lib_path: &Path,
        lsp_server: Option<String>,
        lsp_args: Vec<String>,
    ) -> anyhow::Result<Self> {
        unsafe {
            let lib = libloading::Library::new(lib_path)?;

            // language_id() → static C string (no allocation, no free needed)
            let language_id: String = {
                let sym: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char> =
                    lib.get(b"language_id\0")?;
                let ptr = sym();
                if ptr.is_null() {
                    anyhow::bail!("language_id() returned null");
                }
                CStr::from_ptr(ptr).to_str()?.to_string()
            };

            // file_extensions() → comma-separated static C string (e.g. "py,pyw")
            let extensions: Vec<String> = {
                let sym: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char> =
                    lib.get(b"file_extensions\0")?;
                let ptr = sym();
                if ptr.is_null() {
                    vec![]
                } else {
                    CStr::from_ptr(ptr)
                        .to_str()
                        .unwrap_or("")
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
            };

            // Optional symbols — missing symbols are non-fatal.
            let tokenize_fn = lib
                .get::<unsafe extern "C" fn(*const std::ffi::c_char) -> *mut std::ffi::c_char>(
                    b"tokenize_line_ffi\0",
                )
                .ok()
                .map(|s| *s);

            let free_fn = lib
                .get::<unsafe extern "C" fn(*mut std::ffi::c_char)>(b"free_string\0")
                .ok()
                .map(|s| *s);

            let hover_fn = lib
                .get::<unsafe extern "C" fn(
                    *const std::ffi::c_char,
                    *const std::ffi::c_char,
                ) -> *mut std::ffi::c_char>(b"hover_info_ffi\0")
                .ok()
                .map(|s| *s);

            // We store the library in _lib so it stays alive (symbols remain valid).

            Ok(Self {
                _lib: lib,
                language_id,
                extensions,
                lsp_server,
                lsp_args,
                tokenize_fn,
                free_fn,
                hover_fn,
            })
        }
    }

    fn call_tokenize(&self, line: &str) -> Option<String> {
        let tokenize = self.tokenize_fn?;
        let free = self.free_fn?;
        let c_line = CString::new(line).ok()?;
        unsafe {
            let ptr = tokenize(c_line.as_ptr());
            if ptr.is_null() {
                return None;
            }
            let result = CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string());
            free(ptr);
            result
        }
    }

    fn call_hover(&self, word: &str, content: &str) -> Option<String> {
        let hover = self.hover_fn?;
        let free = self.free_fn?;
        let c_word = CString::new(word).ok()?;
        let c_content = CString::new(content).ok()?;
        unsafe {
            let ptr = hover(c_word.as_ptr(), c_content.as_ptr());
            if ptr.is_null() {
                return None;
            }
            let result = CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string());
            free(ptr);
            result
        }
    }
}

impl Plugin for FfiLangPlugin {
    fn name(&self) -> &str {
        &self.language_id
    }

    fn tokenize_line(&self, lang: &str, line: &str) -> Option<Vec<Token>> {
        if !self.extensions.iter().any(|e| e == lang) {
            return None;
        }
        let json = self.call_tokenize(line)?;
        parse_token_json(&json)
    }

    fn hover_info(&self, lang: &str, word: &str, file_content: &str) -> Option<String> {
        if !self.extensions.iter().any(|e| e == lang) {
            return None;
        }
        self.call_hover(word, file_content)
    }

    fn file_extensions(&self) -> &[&str] {
        // We can't return &[&str] from Vec<String> directly without a self-referential
        // borrow, so we leak a small slice once. Extensions are loaded once per process.
        // This is intentional — extension lifetimes match the process lifetime.
        Box::leak(
            self.extensions
                .iter()
                .map(|s| Box::leak(s.clone().into_boxed_str()) as &str)
                .collect::<Vec<&str>>()
                .into_boxed_slice(),
        )
    }

    fn lsp_server_command(&self) -> Option<(String, Vec<String>)> {
        let server = self.lsp_server.clone()?;
        Some((server, self.lsp_args.clone()))
    }

    fn update(&mut self, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }
}

// ── JSON token parser ─────────────────────────────────────────────────────────

/// Parse the JSON array returned by `tokenize_line_ffi`.
/// Format: `[{"text":"...","kind":"keyword"}, ...]`
fn parse_token_json(json: &str) -> Option<Vec<Token>> {
    let json = json.trim();
    if !json.starts_with('[') || !json.ends_with(']') {
        return None;
    }
    let inner = &json[1..json.len() - 1];
    let mut tokens = Vec::new();
    let mut rest = inner.trim();

    while !rest.is_empty() {
        // Each element is a JSON object: {"text":"...","kind":"..."}
        if !rest.starts_with('{') {
            break;
        }
        let end = rest.find('}')? + 1;
        let obj = &rest[1..end - 1]; // strip braces
        rest = rest[end..].trim_start_matches([',', ' ']);

        let text = extract_json_str(obj, "text")?;
        let kind_str = extract_json_str(obj, "kind")?;
        let kind = match kind_str.as_str() {
            "keyword" => TokenKind::Keyword,
            "type" => TokenKind::KeywordType,
            "string" => TokenKind::String,
            "comment" => TokenKind::Comment,
            "number" => TokenKind::Number,
            "function" => TokenKind::Function,
            "macro" => TokenKind::Macro,
            _ => TokenKind::Normal,
        };
        tokens.push(Token { text, kind });
    }

    Some(tokens)
}

/// Extract a string value for `key` from a flat JSON object body (no nested objects).
fn extract_json_str(obj: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = obj.find(&needle)? + needle.len();
    let rest = obj[start..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..]; // skip opening quote
    let mut result = String::new();
    let mut chars = rest.chars();
    loop {
        match chars.next()? {
            '"' => break,
            '\\' => match chars.next()? {
                '"' => result.push('"'),
                '\\' => result.push('\\'),
                'n' => result.push('\n'),
                'r' => result.push('\r'),
                't' => result.push('\t'),
                c => result.push(c),
            },
            c => result.push(c),
        }
    }
    Some(result)
}
