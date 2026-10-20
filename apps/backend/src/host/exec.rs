use std::time::Duration;

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

use crate::config::Config;

const OUTPUT_CAP: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);

const UNIX_SCRIPT: &str = r#"
cd "$JARVIS_CWD" || exit 97
trap 'code=$?; printf "__JARVIS_CWD__%s\n" "$PWD" >&2; exit $code' EXIT
eval "$JARVIS_CMD"
"#;

const POWERSHELL_SCRIPT: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Location -LiteralPath $env:JARVIS_CWD
$code = 0
try {
  Invoke-Expression $env:JARVIS_CMD
  if ($null -ne $LASTEXITCODE) { $code = $LASTEXITCODE }
} catch {
  [Console]::Error.WriteLine($_.Exception.Message)
  $code = 1
}
[Console]::Error.WriteLine('__JARVIS_CWD__' + (Get-Location).Path)
exit $code
"#;

#[derive(Debug)]
pub struct ExecOutcome {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub cwd: String,
    pub truncated: bool,
    pub timed_out: bool,
    pub exec_mode: String,
}

pub fn exec_mode(config: &Config) -> &'static str {
    if cfg!(unix) && config.host_exec == "host" {
        "host"
    } else {
        "container"
    }
}

pub fn default_cwd(mode: &str) -> String {
    if mode == "host" {
        return "/".into();
    }
    std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| ".".into())
}

pub async fn run(mode: &str, cwd: &str, command: &str) -> ExecOutcome {
    let mut cmd = shell(mode);
    cmd.env("JARVIS_CWD", cwd)
        .env("JARVIS_CMD", command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if mode == "host" {
        let _ = cmd.current_dir("/");
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            return ExecOutcome {
                stdout: String::new(),
                stderr: format!("failed to start shell: {err}"),
                exit_code: None,
                cwd: cwd.to_string(),
                truncated: false,
                timed_out: false,
                exec_mode: mode.to_string(),
            };
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let timed = tokio::time::timeout(TIMEOUT, async move {
        let (out, err, status) = tokio::join!(
            read_capped(stdout),
            read_capped(stderr),
            child.wait()
        );
        (out, err, status)
    })
    .await;

    let (stdout_raw, stderr_raw, status, truncated, timed_out) = match timed {
        Ok(((stdout, out_cut), (stderr, err_cut), status)) => {
            let code = status.ok().and_then(|s| s.code());
            (stdout, stderr, code, out_cut || err_cut, false)
        }
        Err(_) => {
            return ExecOutcome {
                stdout: String::new(),
                stderr: "command timed out after 20s".into(),
                exit_code: None,
                cwd: cwd.to_string(),
                truncated: false,
                timed_out: true,
                exec_mode: mode.to_string(),
            };
        }
    };

    let stdout = String::from_utf8_lossy(&stdout_raw).to_string();
    let stderr_text = String::from_utf8_lossy(&stderr_raw).to_string();
    let (stderr, next_cwd) = take_cwd(&stderr_text, cwd);

    ExecOutcome {
        stdout,
        stderr,
        exit_code: status,
        cwd: next_cwd,
        truncated,
        timed_out,
        exec_mode: mode.to_string(),
    }
}

fn shell(mode: &str) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", POWERSHELL_SCRIPT]);
        return cmd;
    }

    if mode == "host" {
        let mut cmd = Command::new("nsenter");
        cmd.args(["-t", "1", "-m", "-u", "-i", "-n", "-p", "--", "sh", "-c", UNIX_SCRIPT]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", UNIX_SCRIPT]);
        cmd
    }
}

async fn read_capped(reader: Option<impl AsyncRead + Unpin>) -> (Vec<u8>, bool) {
    let Some(mut reader) = reader else {
        return (Vec::new(), false);
    };
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if out.len() >= OUTPUT_CAP {
                    truncated = true;
                    continue;
                }
                let room = OUTPUT_CAP - out.len();
                let take = n.min(room);
                out.extend_from_slice(&buf[..take]);
                if take < n {
                    truncated = true;
                }
            }
        }
    }
    (out, truncated)
}

fn take_cwd(stderr: &str, previous: &str) -> (String, String) {
    let mut lines: Vec<&str> = stderr.split('\n').collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if let Some(last) = lines.last().copied() {
        if let Some(cwd) = last.strip_prefix("__JARVIS_CWD__") {
            let cwd = cwd.trim();
            if !cwd.is_empty() {
                lines.pop();
                return (lines.join("\n"), cwd.to_string());
            }
        }
    }
    (stderr.to_string(), previous.to_string())
}
