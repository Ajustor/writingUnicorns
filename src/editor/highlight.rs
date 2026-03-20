use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent};

// ── Token types ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TokenKind {
    Keyword,
    KeywordType,
    String,
    Comment,
    Number,
    Function,
    Macro,
    Normal,
    Class,     // teal — struct/enum/trait/class names (upper-case identifiers)
    TypeParam, // lighter teal — single-letter generics like T, U
    Operator,
    Property,
}

impl TokenKind {
    pub fn color(self) -> egui::Color32 {
        match self {
            TokenKind::Keyword => egui::Color32::from_rgb(197, 134, 192),
            TokenKind::KeywordType => egui::Color32::from_rgb(86, 156, 214),
            TokenKind::String => egui::Color32::from_rgb(206, 145, 120),
            TokenKind::Comment => egui::Color32::from_rgb(106, 153, 85),
            TokenKind::Number => egui::Color32::from_rgb(181, 206, 168),
            TokenKind::Function => egui::Color32::from_rgb(220, 220, 170),
            TokenKind::Macro => egui::Color32::from_rgb(220, 220, 170),
            TokenKind::Normal => egui::Color32::from_rgb(212, 212, 212),
            TokenKind::Class => egui::Color32::from_rgb(78, 201, 176),
            TokenKind::TypeParam => egui::Color32::from_rgb(180, 220, 220),
            TokenKind::Operator => egui::Color32::from_rgb(212, 212, 212),
            TokenKind::Property => egui::Color32::from_rgb(156, 220, 254),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub text: String,
    pub kind: TokenKind,
}

// ── Tree-sitter highlight name → TokenKind mapping ───────────────────────────

/// The ordered list of capture names recognised by our renderer.
/// Order matters: tree-sitter-highlight resolves captures hierarchically
/// (e.g. "keyword.function" falls back to "keyword" if not listed).
const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",             // 0  → Macro  (#[derive], decorators)
    "comment",               // 1  → Comment
    "comment.documentation", // 2 → Comment
    "constant",              // 3  → Keyword (true / false / nil)
    "constant.builtin",      // 4  → Keyword
    "constructor",           // 5  → Class
    "function",              // 6  → Function
    "function.builtin",      // 7  → Function
    "function.macro",        // 8  → Macro
    "function.method",       // 9  → Function
    "keyword",               // 10 → Keyword
    "keyword.function",      // 11 → Keyword
    "keyword.operator",      // 12 → Operator
    "keyword.return",        // 13 → Keyword
    "keyword.storage",       // 14 → Keyword
    "label",                 // 15 → Normal
    "number",                // 16 → Number
    "operator",              // 17 → Operator
    "property",              // 18 → Property
    "punctuation",           // 19 → Normal
    "punctuation.bracket",   // 20 → Normal
    "punctuation.delimiter", // 21 → Normal
    "string",                // 22 → String
    "string.escape",         // 23 → String
    "string.special",        // 24 → String
    "type",                  // 25 → KeywordType
    "type.builtin",          // 26 → KeywordType
    "variable",              // 27 → Normal
    "variable.builtin",      // 28 → Keyword  (self / this / super)
    "variable.parameter",    // 29 → Normal
];

fn capture_idx_to_kind(idx: usize) -> TokenKind {
    match idx {
        0 => TokenKind::Macro,
        1 | 2 => TokenKind::Comment,
        3 | 4 => TokenKind::Keyword,
        5 => TokenKind::Class,
        6..=9 => TokenKind::Function,
        10 | 11 | 13 | 14 => TokenKind::Keyword,
        12 | 17 => TokenKind::Operator,
        15 => TokenKind::Normal,
        16 => TokenKind::Number,
        18 => TokenKind::Property,
        19..=21 => TokenKind::Normal,
        22..=24 => TokenKind::String,
        25 | 26 => TokenKind::KeywordType,
        27 | 29 => TokenKind::Normal,
        28 => TokenKind::Keyword,
        _ => TokenKind::Normal,
    }
}

/// Normalise an extension to its "primary" tree-sitter language key.
fn primary_lang(lang: &str) -> &str {
    match lang {
        "jsx" | "mjs" | "cjs" => "js",
        _ => lang,
    }
}

// ── Thread-local TS highlighter (reused across frames) ────────────────────────

thread_local! {
    static TS: std::cell::RefCell<tree_sitter_highlight::Highlighter> =
        std::cell::RefCell::new(tree_sitter_highlight::Highlighter::new());
}

// ── Highlighter ───────────────────────────────────────────────────────────────

pub struct Highlighter {
    pub language: String,
    /// Pre-computed token list per line, rebuilt when `content_version` changes.
    line_tokens: Vec<Vec<Token>>,
    /// The `content_version` for which `line_tokens` was last computed.
    last_version: i32,
    /// Tree-sitter configurations, keyed by primary language extension.
    configs: std::collections::HashMap<String, HighlightConfiguration>,
}

impl Highlighter {
    pub fn new() -> Self {
        let mut configs = std::collections::HashMap::new();

        try_add_config(
            &mut configs,
            "rs",
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
            "",
        );

        try_add_config(
            &mut configs,
            "js",
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        );

        try_add_config(
            &mut configs,
            "py",
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
        );

        Self {
            language: String::new(),
            line_tokens: Vec::new(),
            last_version: -1,
            configs,
        }
    }

    pub fn set_language(&mut self, ext: &str) {
        let new_lang = ext.to_lowercase();
        if self.language != new_lang {
            self.language = new_lang;
            self.last_version = -1; // force re-highlight on next frame
            self.line_tokens.clear();
        }
    }

    pub fn set_language_from_filename(&mut self, filename: &str) {
        let lower = filename.to_lowercase();
        if lower == "dockerfile" || lower.starts_with("dockerfile.") {
            self.set_language("dockerfile");
            return;
        }
        if lower == "makefile" || lower == "gnumakefile" {
            self.set_language("makefile");
            return;
        }
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
        self.set_language(&ext);
    }

    /// Returns true when the cached tokens are stale and need rebuilding.
    pub fn needs_update(&self, version: i32) -> bool {
        version != self.last_version
    }

    /// Rebuild the per-line token cache from `source`.
    /// No-op if `version` matches the last-computed version.
    pub fn highlight_document(&mut self, source: &str, version: i32) {
        if version == self.last_version {
            return;
        }
        self.last_version = version;

        let primary = primary_lang(&self.language).to_string();
        if let Some(config) = self.configs.get(&primary) {
            match ts_highlight(config, source) {
                Ok(spans) => {
                    self.line_tokens = spans;
                    return;
                }
                Err(e) => {
                    log::debug!("tree-sitter highlight error for {}: {e:?}", self.language);
                }
            }
        }

        // Regex fallback (unsupported languages, or TS parse error)
        let lang = self.language.clone();
        self.line_tokens = source
            .lines()
            .map(|line| tokenize_line_regex(&lang, line))
            .collect();
    }

    /// Return tokens for `line_idx`, falling back to on-the-fly regex if the
    /// cache doesn't cover that line.
    pub fn tokens_for_line(&self, line_idx: usize, line_text: &str) -> Vec<Token> {
        match self.line_tokens.get(line_idx) {
            Some(toks) if !toks.is_empty() => toks.clone(),
            _ => self.tokenize_line(line_text),
        }
    }

    /// Single-line regex tokeniser kept for backward-compat (plugins, hover,
    /// folded preview when the tree-sitter cache isn't available).
    pub fn tokenize_line(&self, line: &str) -> Vec<Token> {
        tokenize_line_regex(&self.language, line)
    }
}

// ── Tree-sitter highlight engine ─────────────────────────────────────────────

fn try_add_config(
    map: &mut std::collections::HashMap<String, HighlightConfiguration>,
    key: &str,
    language: tree_sitter::Language,
    highlights: &str,
    injections: &str,
    locals: &str,
) {
    match HighlightConfiguration::new(language, key, highlights, injections, locals) {
        Ok(mut cfg) => {
            cfg.configure(HIGHLIGHT_NAMES);
            map.insert(key.to_string(), cfg);
        }
        Err(e) => log::warn!("Failed to build tree-sitter config for {key}: {e:?}"),
    }
}

/// Run tree-sitter-highlight on `source` and return per-line token vectors.
fn ts_highlight(
    config: &HighlightConfiguration,
    source: &str,
) -> Result<Vec<Vec<Token>>, tree_sitter_highlight::Error> {
    let source_bytes = source.as_bytes();
    let num_lines = source.lines().count().max(1);
    let mut result: Vec<Vec<Token>> = vec![Vec::new(); num_lines];

    // Build a lookup table: byte offset → line index.
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source_bytes.iter().enumerate().filter_map(|(i, &b)| {
            if b == b'\n' {
                Some(i + 1)
            } else {
                None
            }
        }))
        .collect();

    // Collect all highlight events before releasing the thread-local borrow.
    let events: Vec<HighlightEvent> = TS.with(|cell| {
        let mut hl = cell.borrow_mut();
        let result = hl
            .highlight(config, source_bytes, None, |_| None)?
            .collect::<Result<Vec<_>, _>>();
        result
    })?;

    // Replay events to build per-line token lists.
    let mut kind_stack: Vec<TokenKind> = Vec::new();

    for event in events {
        match event {
            HighlightEvent::HighlightStart(h) => {
                kind_stack.push(capture_idx_to_kind(h.0));
            }
            HighlightEvent::HighlightEnd => {
                kind_stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                if start >= end {
                    continue;
                }
                let kind = kind_stack.last().copied().unwrap_or(TokenKind::Normal);
                let text = match source.get(start..end) {
                    Some(t) => t,
                    None => continue,
                };

                // The span may cover multiple lines — split it.
                let start_line = line_starts
                    .partition_point(|&s| s <= start)
                    .saturating_sub(1);
                let mut line_idx = start_line;

                for piece in text.split('\n') {
                    if line_idx < result.len() && !piece.is_empty() {
                        result[line_idx].push(Token {
                            text: piece.to_string(),
                            kind,
                        });
                    }
                    line_idx += 1;
                }
            }
        }
    }

    // Any line still empty gets its raw text as a single Normal token.
    for (tokens, line_text) in result.iter_mut().zip(source.lines()) {
        if tokens.is_empty() && !line_text.is_empty() {
            tokens.push(Token {
                text: line_text.to_string(),
                kind: TokenKind::Normal,
            });
        }
    }

    Ok(result)
}

// ── Regex-based fallback tokenisers ──────────────────────────────────────────

fn tokenize_line_regex(lang: &str, line: &str) -> Vec<Token> {
    match lang {
        "rs" => tokenize_rust(line),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" => tokenize_js_ts(line),
        "py" => tokenize_python(line),
        "json" => tokenize_json(line),
        "toml" => tokenize_toml(line),
        "sh" | "bash" | "zsh" => tokenize_shell(line),
        "dockerfile" => tokenize_dockerfile(line),
        _ => vec![Token {
            text: line.to_string(),
            kind: TokenKind::Normal,
        }],
    }
}

fn push_normal(tokens: &mut Vec<Token>, text: &str) {
    if !text.is_empty() {
        tokens.push(Token {
            text: text.to_string(),
            kind: TokenKind::Normal,
        });
    }
}

fn generic_tokenize(
    line: &str,
    comment_prefix: &str,
    string_chars: &[char],
    keywords: &[&str],
    type_keywords: &[&str],
) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Line comment
        if line[char_byte_offset(&chars, i)..].starts_with(comment_prefix) {
            tokens.push(Token {
                text: line[char_byte_offset(&chars, i)..].to_string(),
                kind: TokenKind::Comment,
            });
            return tokens;
        }

        // String literal
        if string_chars.contains(&chars[i]) {
            let quote = chars[i];
            let mut s = String::new();
            s.push(quote);
            i += 1;
            while i < len {
                let c = chars[i];
                s.push(c);
                i += 1;
                if c == '\\' && i < len {
                    s.push(chars[i]);
                    i += 1;
                } else if c == quote {
                    break;
                }
            }
            tokens.push(Token {
                text: s,
                kind: TokenKind::String,
            });
            continue;
        }

        // Number
        if chars[i].is_ascii_digit()
            || (chars[i] == '-' && i + 1 < len && chars[i + 1].is_ascii_digit())
        {
            let mut s = String::new();
            if chars[i] == '-' {
                s.push('-');
                i += 1;
            }
            while i < len
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_')
            {
                s.push(chars[i]);
                i += 1;
            }
            tokens.push(Token {
                text: s,
                kind: TokenKind::Number,
            });
            continue;
        }

        // Word (identifier or keyword)
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let followed_by_paren = i < len && chars[i] == '(';
            let preceded_by_dot = start > 0 && chars[start - 1] == '.';
            let first_char = word.chars().next().unwrap_or('\0');
            let kind = if type_keywords.contains(&word.as_str()) {
                TokenKind::KeywordType
            } else if keywords.contains(&word.as_str()) {
                TokenKind::Keyword
            } else if followed_by_paren {
                TokenKind::Function
            } else if preceded_by_dot && word.chars().all(|c| c.is_lowercase() || c == '_') {
                TokenKind::Property
            } else if first_char.is_uppercase() && word.chars().count() == 1 {
                TokenKind::TypeParam
            } else if first_char.is_uppercase() {
                TokenKind::Class
            } else {
                TokenKind::Normal
            };
            tokens.push(Token { text: word, kind });
            continue;
        }

        // Everything else: non-word characters
        let start = i;
        while i < len
            && !chars[i].is_alphabetic()
            && chars[i] != '_'
            && !chars[i].is_ascii_digit()
            && !string_chars.contains(&chars[i])
            && !line[char_byte_offset(&chars, i)..].starts_with(comment_prefix)
        {
            i += 1;
        }
        if i > start {
            push_normal(&mut tokens, &chars[start..i].iter().collect::<String>());
        }
    }
    tokens
}

fn char_byte_offset(chars: &[char], char_idx: usize) -> usize {
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}

pub fn tokenize_rust(line: &str) -> Vec<Token> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("///") || trimmed.starts_with("//!") {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Comment,
        }];
    }
    let mut tokens = generic_tokenize(
        line,
        "//",
        &['"', '\''],
        &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while",
        ],
        &[
            "bool", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128",
            "isize", "f32", "f64", "char", "str", "String", "Vec", "Option", "Result", "Box",
            "Arc", "Rc", "Mutex", "HashMap", "HashSet", "BTreeMap", "BTreeSet",
        ],
    );
    let src_chars: Vec<char> = line.chars().collect();
    let mut col = 0usize;
    for tok in &mut tokens {
        let end = col + tok.text.chars().count();
        if (tok.kind == TokenKind::Normal
            || tok.kind == TokenKind::Function
            || tok.kind == TokenKind::Class
            || tok.kind == TokenKind::TypeParam)
            && end < src_chars.len()
            && src_chars[end] == '!'
        {
            tok.kind = TokenKind::Macro;
        }
        col = end;
    }
    tokens
}

pub fn tokenize_js_ts(line: &str) -> Vec<Token> {
    generic_tokenize(
        line,
        "//",
        &['"', '\'', '`'],
        &[
            "abstract",
            "arguments",
            "as",
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "enum",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "from",
            "function",
            "get",
            "if",
            "implements",
            "import",
            "in",
            "instanceof",
            "interface",
            "let",
            "module",
            "namespace",
            "new",
            "null",
            "of",
            "package",
            "private",
            "protected",
            "public",
            "readonly",
            "return",
            "set",
            "static",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "type",
            "typeof",
            "undefined",
            "var",
            "void",
            "while",
            "with",
            "yield",
            "declare",
            "keyof",
            "infer",
            "never",
            "unknown",
            "any",
            "override",
            "satisfies",
            "accessor",
        ],
        &[
            "boolean",
            "number",
            "string",
            "symbol",
            "bigint",
            "object",
            "Array",
            "Promise",
            "Record",
            "Partial",
            "Required",
            "Readonly",
            "Pick",
            "Omit",
            "Exclude",
            "Extract",
            "NonNullable",
            "ReturnType",
            "InstanceType",
            "Map",
            "Set",
            "Date",
            "RegExp",
            "Error",
        ],
    )
}

pub fn tokenize_python(line: &str) -> Vec<Token> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Comment,
        }];
    }
    generic_tokenize(
        line,
        "#",
        &['"', '\''],
        &[
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "try", "while", "with", "yield",
        ],
        &[
            "int",
            "float",
            "str",
            "bool",
            "bytes",
            "list",
            "dict",
            "set",
            "tuple",
            "type",
            "object",
            "super",
            "property",
            "classmethod",
            "staticmethod",
            "Exception",
            "TypeError",
            "ValueError",
            "KeyError",
            "IndexError",
            "AttributeError",
        ],
    )
}

pub fn tokenize_json(line: &str) -> Vec<Token> {
    generic_tokenize(line, "//", &['"'], &["true", "false", "null"], &[])
}

pub fn tokenize_toml(line: &str) -> Vec<Token> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Comment,
        }];
    }
    if trimmed.starts_with('[') {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::KeywordType,
        }];
    }
    generic_tokenize(line, "#", &['"', '\''], &["true", "false"], &[])
}

pub fn tokenize_shell(line: &str) -> Vec<Token> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Comment,
        }];
    }
    generic_tokenize(
        line,
        "#",
        &['"', '\''],
        &[
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
            "function", "return", "exit", "export", "local", "readonly", "source", "echo", "cd",
            "ls", "mkdir", "rm", "cp", "mv", "cat", "grep", "sed", "awk", "find", "chmod", "chown",
            "sudo", "true", "false", "in",
        ],
        &[],
    )
}

pub fn tokenize_dockerfile(line: &str) -> Vec<Token> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Comment,
        }];
    }
    let instructions = [
        "FROM",
        "RUN",
        "CMD",
        "LABEL",
        "EXPOSE",
        "ENV",
        "ADD",
        "COPY",
        "ENTRYPOINT",
        "VOLUME",
        "USER",
        "WORKDIR",
        "ARG",
        "ONBUILD",
        "STOPSIGNAL",
        "HEALTHCHECK",
        "SHELL",
        "MAINTAINER",
    ];
    let upper = trimmed.to_uppercase();
    for instr in &instructions {
        if upper == *instr
            || upper.starts_with(&format!("{} ", instr))
            || upper.starts_with(&format!("{}\t", instr))
        {
            let instr_len = instr.len();
            let leading = line.len() - trimmed.len();
            let mut tokens = Vec::new();
            if leading > 0 {
                tokens.push(Token {
                    text: line[..leading].to_string(),
                    kind: TokenKind::Normal,
                });
            }
            tokens.push(Token {
                text: line[leading..leading + instr_len].to_string(),
                kind: TokenKind::Keyword,
            });
            let rest = &line[leading + instr_len..];
            if !rest.is_empty() {
                let rest_tokens = generic_tokenize(rest, "#", &['"', '\''], &["AS", "as"], &[]);
                tokens.extend(rest_tokens);
            }
            return tokens;
        }
    }
    generic_tokenize(line, "#", &['"', '\''], &["AS", "as", "true", "false"], &[])
}

/// Returns all keywords for a given language (used by autocomplete).
pub fn keywords_for_language(lang: &str) -> &'static [&'static str] {
    match lang {
        "rs" => &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while", "bool", "u8", "u16", "u32",
            "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64",
            "char", "str", "String", "Vec", "Option", "Result", "Box", "Arc", "Rc", "Mutex",
            "HashMap", "HashSet", "BTreeMap", "BTreeSet",
        ],
        "js" | "ts" | "jsx" | "tsx" | "mjs" => &[
            "abstract",
            "arguments",
            "as",
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "enum",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "from",
            "function",
            "get",
            "if",
            "implements",
            "import",
            "in",
            "instanceof",
            "interface",
            "let",
            "module",
            "namespace",
            "new",
            "null",
            "of",
            "package",
            "private",
            "protected",
            "public",
            "readonly",
            "return",
            "set",
            "static",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "type",
            "typeof",
            "undefined",
            "var",
            "void",
            "while",
            "with",
            "yield",
            "declare",
            "keyof",
            "infer",
            "never",
            "unknown",
            "any",
            "override",
            "satisfies",
            "accessor",
            "boolean",
            "number",
            "string",
            "symbol",
            "bigint",
            "object",
            "Array",
            "Promise",
            "Record",
            "Partial",
            "Required",
            "Readonly",
            "Pick",
            "Omit",
            "Exclude",
            "Extract",
            "NonNullable",
            "ReturnType",
            "InstanceType",
            "Map",
            "Set",
            "Date",
            "RegExp",
            "Error",
        ],
        "py" => &[
            "False",
            "None",
            "True",
            "and",
            "as",
            "assert",
            "async",
            "await",
            "break",
            "class",
            "continue",
            "def",
            "del",
            "elif",
            "else",
            "except",
            "finally",
            "for",
            "from",
            "global",
            "if",
            "import",
            "in",
            "is",
            "lambda",
            "nonlocal",
            "not",
            "or",
            "pass",
            "raise",
            "return",
            "try",
            "while",
            "with",
            "yield",
            "int",
            "float",
            "str",
            "bool",
            "bytes",
            "list",
            "dict",
            "set",
            "tuple",
            "type",
            "object",
            "super",
            "property",
            "classmethod",
            "staticmethod",
            "Exception",
            "TypeError",
            "ValueError",
            "KeyError",
            "IndexError",
            "AttributeError",
        ],
        "sh" | "bash" | "zsh" => &[
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
            "function", "return", "exit", "export", "local", "readonly", "source", "echo", "cd",
            "ls", "mkdir", "rm", "cp", "mv", "cat", "grep", "sed", "awk", "find", "chmod", "chown",
            "sudo", "true", "false", "in",
        ],
        "dockerfile" => &[
            "FROM",
            "RUN",
            "CMD",
            "LABEL",
            "EXPOSE",
            "ENV",
            "ADD",
            "COPY",
            "ENTRYPOINT",
            "VOLUME",
            "USER",
            "WORKDIR",
            "ARG",
            "ONBUILD",
            "STOPSIGNAL",
            "HEALTHCHECK",
            "SHELL",
            "MAINTAINER",
            "AS",
            "as",
        ],
        "json" => &["true", "false", "null"],
        "toml" => &["true", "false"],
        _ => &[],
    }
}
