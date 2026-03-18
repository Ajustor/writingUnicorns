use std::path::PathBuf;

/// Configuration for a Debug Adapter, returned by language plugins.
#[derive(Debug, Clone)]
pub struct DapConfig {
    /// The debug adapter binary (e.g. "codelldb", "python3", "node").
    pub adapter_cmd: String,
    /// Arguments for the adapter (e.g. ["-m", "debugpy.adapter"]).
    pub adapter_args: Vec<String>,
    /// The `launch` request body sent after `configurationDone`.
    /// Use `${file}` and `${workspaceFolder}` as placeholders.
    pub launch_config: serde_json::Value,
}

/// A source breakpoint (before or after DAP verification).
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub file: PathBuf,
    /// 1-based line number.
    pub line: usize,
    /// Set to true once the DAP server acknowledges it.
    pub verified: bool,
    pub id: Option<i64>,
}

/// One frame in the call stack.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub id: i64,
    pub name: String,
    pub file: Option<PathBuf>,
    /// 1-based line number.
    pub line: usize,
}

/// A debug variable (or scope entry).
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub var_type: Option<String>,
    /// Non-zero when this variable can be expanded (has children).
    pub variables_reference: i64,
}

/// Lifecycle state of the active debug session.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum DebugSessionState {
    #[default]
    Idle,
    Launching,
    Running,
    Paused {
        thread_id: i64,
    },
    Terminated,
}
