use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::client::DapClient;
use super::types::{DapConfig, DebugSessionState, StackFrame, Variable};

/// Manages the active debug session and breakpoint storage.
#[derive(Default)]
pub struct DapManager {
    pub session: Option<DapClient>,
    /// Breakpoints per file: file path → set of 1-based line numbers.
    pub breakpoints: HashMap<PathBuf, HashSet<usize>>,
    /// Set when a pause just happened — caller may want to navigate to the top frame.
    pub just_paused: bool,
}

impl DapManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle a breakpoint at the given file/line. Returns the new set for this file.
    pub fn toggle_breakpoint(&mut self, file: &Path, line: usize) -> Vec<usize> {
        let set = self.breakpoints.entry(file.to_path_buf()).or_default();
        if set.contains(&line) {
            set.remove(&line);
        } else {
            set.insert(line);
        }
        let mut lines: Vec<usize> = set.iter().cloned().collect();
        lines.sort_unstable();
        // Sync with active session if any.
        if let Some(sess) = &mut self.session {
            sess.set_breakpoints(file, &lines);
        }
        lines
    }

    /// Returns the sorted breakpoint lines for a given file (0-based for display comparison).
    pub fn breakpoint_lines_for(&self, file: &Path) -> HashSet<usize> {
        self.breakpoints.get(file).cloned().unwrap_or_default()
    }

    /// Start a new debug session.
    pub fn start_session(
        &mut self,
        cfg: &DapConfig,
        workspace: &Path,
        current_file: Option<&Path>,
    ) -> anyhow::Result<()> {
        let mut client = DapClient::start(cfg, workspace)?;
        // If a current file is set, substitute ${file} in the launch config.
        if let Some(file) = current_file {
            client.set_file_variable(file);
        }
        // Queue all stored breakpoints.
        let bps: Vec<(PathBuf, Vec<usize>)> = self
            .breakpoints
            .iter()
            .map(|(f, ls)| {
                let mut lines: Vec<usize> = ls.iter().cloned().collect();
                lines.sort_unstable();
                (f.clone(), lines)
            })
            .collect();
        for (file, lines) in &bps {
            client.set_breakpoints(file, lines);
        }
        self.session = Some(client);
        Ok(())
    }

    /// Stop the active debug session.
    pub fn stop_session(&mut self) {
        if let Some(sess) = &mut self.session {
            sess.disconnect();
        }
        self.session = None;
    }

    /// Poll the active session. Call every frame.
    pub fn poll(&mut self) {
        self.just_paused = false;
        if let Some(sess) = &mut self.session {
            let paused = sess.poll();
            if paused {
                self.just_paused = true;
            }
            // Clean up terminated sessions automatically.
            if sess.state == DebugSessionState::Terminated && !sess.is_alive() {
                // Keep the session alive a bit so the UI can show the final state;
                // layout.rs is responsible for calling stop_session() on user action.
            }
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.session.as_ref().map(|s| &s.state),
            Some(DebugSessionState::Running) | Some(DebugSessionState::Launching)
        )
    }

    pub fn is_paused(&self) -> bool {
        matches!(
            self.session.as_ref().map(|s| &s.state),
            Some(DebugSessionState::Paused { .. })
        )
    }

    pub fn is_active(&self) -> bool {
        self.session.is_some()
    }

    pub fn paused_thread_id(&self) -> Option<i64> {
        match self.session.as_ref()?.state {
            DebugSessionState::Paused { thread_id } => Some(thread_id),
            _ => None,
        }
    }

    pub fn call_stack(&self) -> &[StackFrame] {
        self.session
            .as_ref()
            .map(|s| s.call_stack.as_slice())
            .unwrap_or(&[])
    }

    pub fn variables(&self) -> &[Variable] {
        self.session
            .as_ref()
            .map(|s| s.variables.as_slice())
            .unwrap_or(&[])
    }

    pub fn output_log(&self) -> &[String] {
        self.session
            .as_ref()
            .map(|s| s.output_log.as_slice())
            .unwrap_or(&[])
    }

    pub fn session_state(&self) -> DebugSessionState {
        self.session
            .as_ref()
            .map(|s| s.state.clone())
            .unwrap_or(DebugSessionState::Idle)
    }
}
