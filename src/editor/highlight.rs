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
        }
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub text: String,
    pub kind: TokenKind,
}

pub struct Highlighter {
    pub language: String,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            language: String::new(),
        }
    }

    pub fn set_language(&mut self, ext: &str) {
        self.language = ext.to_lowercase();
    }

    pub fn set_language_from_filename(&mut self, filename: &str) {
        let lower = filename.to_lowercase();
        // Special filenames without extension
        if lower == "dockerfile" || lower.starts_with("dockerfile.") {
            self.language = "dockerfile".to_string();
            return;
        }
        if lower == "makefile" || lower == "gnumakefile" {
            self.language = "makefile".to_string();
            return;
        }
        // Use extension
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
        self.language = ext;
    }

    pub fn tokenize_line(&self, line: &str) -> Vec<Token> {
        match self.language.as_str() {
            "rs" => tokenize_rust(line),
            "js" | "ts" | "jsx" | "tsx" | "mjs" => tokenize_js_ts(line),
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
            let kind = if type_keywords.contains(&word.as_str()) {
                TokenKind::KeywordType
            } else if keywords.contains(&word.as_str()) {
                TokenKind::Keyword
            } else if followed_by_paren {
                TokenKind::Function
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

/// Convert a char index into a byte offset for string slicing.
fn char_byte_offset(chars: &[char], char_idx: usize) -> usize {
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}

fn tokenize_rust(line: &str) -> Vec<Token> {
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
    // Mark macro calls: word followed by '!'
    let src_chars: Vec<char> = line.chars().collect();
    let mut col = 0usize;
    for tok in &mut tokens {
        let end = col + tok.text.chars().count();
        if (tok.kind == TokenKind::Normal || tok.kind == TokenKind::Function)
            && end < src_chars.len()
            && src_chars[end] == '!'
        {
            tok.kind = TokenKind::Macro;
        }
        col = end;
    }
    tokens
}

fn tokenize_js_ts(line: &str) -> Vec<Token> {
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

fn tokenize_python(line: &str) -> Vec<Token> {
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

fn tokenize_json(line: &str) -> Vec<Token> {
    generic_tokenize(line, "//", &['"'], &["true", "false", "null"], &[])
}

fn tokenize_toml(line: &str) -> Vec<Token> {
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

fn tokenize_shell(line: &str) -> Vec<Token> {
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

fn tokenize_dockerfile(line: &str) -> Vec<Token> {
    let trimmed = line.trim_start();
    // Comments
    if trimmed.starts_with('#') {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Comment,
        }];
    }
    // Dockerfile instructions are at line start (may have leading spaces in multi-stage)
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
    // Check if line starts with a known instruction
    let upper = trimmed.to_uppercase();
    for instr in &instructions {
        if upper == *instr
            || upper.starts_with(&format!("{} ", instr))
            || upper.starts_with(&format!("{}\t", instr))
        {
            let instr_len = instr.len();
            // Find leading whitespace length
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
                // Highlight strings in the rest
                let rest_tokens = generic_tokenize(rest, "#", &['"', '\''], &["AS", "as"], &[]);
                tokens.extend(rest_tokens);
            }
            return tokens;
        }
    }
    // Continuation lines (RUN \ multiline) — treat as shell-like
    generic_tokenize(line, "#", &['"', '\''], &["AS", "as", "true", "false"], &[])
}
