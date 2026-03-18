/// Returns `(shell_path, extra_args)` for the current OS.
pub(super) fn resolve_shell() -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        let ps =
            std::path::Path::new("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe");
        if ps.exists() {
            return (
                "powershell.exe".to_string(),
                vec!["powershell.exe".to_string(), "-NoExit".to_string()],
            );
        }
        (
            "cmd.exe".to_string(),
            vec!["cmd.exe".to_string(), "/K".to_string()],
        )
    }

    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                if std::path::Path::new("/bin/bash").exists() {
                    Some("/bin/bash".to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "/bin/sh".to_string());
        let args = vec![shell.clone(), "-l".to_string()];
        (shell, args)
    }
}
