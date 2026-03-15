use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub line: u32,
    pub col: u32,
    pub severity: DiagSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiagSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub kind: String,
}

pub struct LspClient {
    pub diagnostics: HashMap<String, Vec<Diagnostic>>,
    pub completions: Vec<CompletionItem>,
    pub is_connected: bool,
}

impl LspClient {
    pub fn new() -> Self {
        Self {
            diagnostics: HashMap::new(),
            completions: vec![],
            is_connected: false,
        }
    }

    pub fn get_diagnostics(&self, path: &str) -> &[Diagnostic] {
        self.diagnostics
            .get(path)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
