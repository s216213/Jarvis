//! Launch a named application. The target is data for the OS, never a shell command.

use std::path::{Component, Path};

use chrono::{Duration as ChronoDuration, Utc};
use uuid::Uuid;

use crate::{db::DbPool, error::{AppError, AppResult}};

const CLIENT_MARKER: &str = "[[launch:";
const VM_MARKER: &str = "[[launch-vm:";
const HANDOFF_MARKER: &str = "[[handoff:";
const MARKER_CLOSE: &str = "]]";
const MAX_LAUNCHES: usize = 3;

#[derive(Clone, Debug)]
pub struct ConnectedDevice {
    pub id: Uuid,
    pub name: String,
    pub platform: String,
    pub system_name: String,
    pub fingerprint: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PlannedLaunch {
    pub app: String,
    pub on_vm: bool,
}

#[derive(Debug)]
pub struct ClaimedLaunch {
    pub id: Uuid,
    pub app: String,
}

pub struct ClaimedWork {
    pub launches: Vec<ClaimedLaunch>,
    pub handoff: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum MarkerKind {
    Client,
    Vm,
    Handoff,
}

pub struct MarkerPlan {
    pub text: String,
    pub launches: Vec<PlannedLaunch>,
    pub handoff: Option<String>,
}

pub fn take_launches(reply: &str) -> MarkerPlan {
    let mut apps = Vec::new();
    let mut handoff = None;
    let mut visible = String::new();
    let mut rest = reply;
    while let Some((start, kind, open_len)) = next_marker(rest) {
        visible.push_str(&rest[..start]);
        let after = &rest[start + open_len..];
        let Some(end) = after.find(MARKER_CLOSE) else {
            visible.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let raw = &after[..end];
        match kind {
            MarkerKind::Handoff => {
                if handoff.is_none() {
                    handoff = handoff_token(raw);
                }
            }
            MarkerKind::Client | MarkerKind::Vm if apps.len() < MAX_LAUNCHES => {
                let on_vm = kind == MarkerKind::Vm;
                let normalized = if on_vm {
                    normalize_vm_app(raw)
                } else {
                    normalize_app(raw)
                };
                if let Ok(app) = normalized {
                    let planned = PlannedLaunch { app, on_vm };
                    if !apps.contains(&planned) {
                        apps.push(planned);
                    }
                }
            }
            MarkerKind::Client | MarkerKind::Vm => {}
        }
        rest = &after[end + MARKER_CLOSE.len()..];
    }
    visible.push_str(rest);
    MarkerPlan {
        text: tidy(&visible),
        launches: apps,
        handoff,
    }
}

fn next_marker(rest: &str) -> Option<(usize, MarkerKind, usize)> {
    let candidates = [
        rest.find(VM_MARKER).map(|at| (at, MarkerKind::Vm, VM_MARKER.len())),
        rest.find(HANDOFF_MARKER)
            .map(|at| (at, MarkerKind::Handoff, HANDOFF_MARKER.len())),
        rest.find(CLIENT_MARKER)
            .map(|at| (at, MarkerKind::Client, CLIENT_MARKER.len())),
    ];
    candidates.into_iter().flatten().min_by_key(|item| item.0)
}

fn handoff_token(raw: &str) -> Option<String> {
    let token = raw.trim();
    if token.is_empty() || token.chars().count() > 80 {
        return None;
    }
    if token.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                '[' | ']' | '&' | '|' | ';' | '`' | '$' | '<' | '>' | '"' | '\''
            )
    }) {
        return None;
    }
    Some(token.to_string())
}

pub fn is_windows_only(app: &str) -> bool {
    let lower = app.to_lowercase();
    if lower.starts_with("ms-settings:") {
        return true;
    }
    let bytes = app.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        return true;
    }
    matches!(
        lower.as_str(),
        "notepad" | "calc" | "mspaint" | "explorer" | "wt" | "msedge" | "powershell" | "cmd"
    )
}

pub fn resolve_handoff<'a>(
    devices: &'a [ConnectedDevice],
    token: &str,
) -> Option<&'a ConnectedDevice> {
    let token = token.trim();
    if let Some(found) = devices
        .iter()
        .find(|device| device.fingerprint.eq_ignore_ascii_case(token))
    {
        return Some(found);
    }
    if token.len() >= 8 {
        let prefix = token.to_ascii_lowercase();
        if let Some(found) = devices
            .iter()
            .find(|device| device.fingerprint.to_ascii_lowercase().starts_with(&prefix))
        {
            return Some(found);
        }
    }
    let lower = token.to_lowercase();
    let named: Vec<_> = devices
        .iter()
        .filter(|device| device.name.eq_ignore_ascii_case(token))
        .collect();
    if named.len() == 1 {
        return Some(named[0]);
    }
    let platform: Vec<_> = devices
        .iter()
        .filter(|device| device.platform.eq_ignore_ascii_case(&lower))
        .collect();
    if platform.len() == 1 {
        Some(platform[0])
    } else {
        None
    }
}

pub fn is_windows_desktop(device: &ConnectedDevice) -> bool {
    device.platform == "desktop" && device.system_name.eq_ignore_ascii_case("windows")
}

pub fn can_launch(device: &ConnectedDevice) -> bool {
    device.platform == "desktop" || device.platform == "android"
}

pub fn windows_desktop(devices: &[ConnectedDevice]) -> Option<&ConnectedDevice> {
    devices.iter().find(|device| is_windows_desktop(device))
}

/// Prefer the most recently seen desktop, then a phone. The device list is already newest first.
pub fn capable_device(devices: &[ConnectedDevice]) -> Option<&ConnectedDevice> {
    devices
        .iter()
        .find(|device| device.platform == "desktop")
        .or_else(|| devices.iter().find(|device| device.platform == "android"))
}

pub struct HandoffDecision<'a> {
    pub target: Option<&'a ConnectedDevice>,
    pub unresolved: bool,
    pub windows_needed: bool,
}

/// Where this turn must finish. An explicit marker wins. Otherwise a program the sender
/// cannot run is moved to the device that can.
pub fn choose_handoff<'a>(
    devices: &'a [ConnectedDevice],
    sender: Option<&'a ConnectedDevice>,
    requested: Option<&str>,
    launches: &[PlannedLaunch],
) -> HandoffDecision<'a> {
    let mut target = requested.and_then(|token| resolve_handoff(devices, token));
    let unresolved = requested.is_some() && target.is_none();
    let windows_needed = launches
        .iter()
        .any(|item| !item.on_vm && is_windows_only(&item.app));
    let client_launch = launches.iter().any(|item| !item.on_vm);
    if target.is_none() && windows_needed {
        let sender_is_windows = sender.is_some_and(is_windows_desktop);
        target = if sender_is_windows {
            sender
        } else {
            windows_desktop(devices)
        };
    }
    if target.is_none() && client_launch && !sender.is_some_and(can_launch) {
        target = capable_device(devices);
    }
    HandoffDecision {
        target,
        unresolved,
        windows_needed,
    }
}

pub fn normalize_app(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim().trim_matches('"').trim();
    if trimmed.is_empty() || trimmed.chars().count() > 240 {
        return Err("invalid app");
    }
    if trimmed.chars().any(char::is_control) {
        return Err("invalid app");
    }
    let lower = trimmed.to_lowercase();
    if let Some(aliased) = alias(&lower) {
        return Ok(aliased.to_string());
    }
    if is_settings_uri(&lower) || is_http_url(trimmed) {
        return Ok(if is_settings_uri(&lower) {
            lower
        } else {
            trimmed.to_string()
        });
    }
    if looks_like_path(trimmed) {
        return normalize_path(trimmed);
    }
    if is_bare_name(trimmed) {
        return Ok(trimmed.to_string());
    }
    Err("invalid app")
}

pub async fn connected_devices(pool: &DbPool, user_id: Uuid) -> AppResult<Vec<ConnectedDevice>> {
    let since = Utc::now() - ChronoDuration::hours(24);
    let rows: Vec<(Uuid, String, String, String, String)> = sqlx::query_as(
        r#"SELECT id, name, platform, system_name, fingerprint
           FROM devices
           WHERE user_id = $1 AND is_revoked = $2 AND last_seen_at >= $3
           ORDER BY last_seen_at DESC
           LIMIT 8"#,
    )
    .bind(user_id)
    .bind(false)
    .bind(since)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal(format!("list connected devices: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(id, name, platform, system_name, fingerprint)| ConnectedDevice {
            id,
            name,
            platform,
            system_name,
            fingerprint,
        })
        .collect())
}

pub async fn enqueue_client(
    pool: &DbPool,
    user_id: Uuid,
    app: &str,
    device_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO client_launches (id, user_id, app, created_at, device_id)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(app)
    .bind(Utc::now())
    .bind(device_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::internal(format!("enqueue launch: {e}")))?;
    Ok(())
}

pub async fn enqueue_handoff(
    pool: &DbPool,
    user_id: Uuid,
    device_id: Uuid,
    content: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO device_handoffs (id, user_id, device_id, content, created_at)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(device_id)
    .bind(content)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|e| AppError::internal(format!("enqueue handoff: {e}")))?;
    Ok(())
}

pub async fn capable_clients(pool: &DbPool, user_id: Uuid) -> AppResult<i64> {
    let (count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM devices
           WHERE user_id = $1 AND is_revoked = $2 AND platform IN ('desktop', 'android')"#,
    )
    .bind(user_id)
    .bind(false)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("count clients: {e}")))?;
    Ok(count)
}

pub async fn claim_client(pool: &DbPool, user_id: Uuid, fingerprint: &str) -> AppResult<ClaimedWork> {
    let device: Option<(Uuid, String)> = sqlx::query_as(
        r#"SELECT id, platform FROM devices
           WHERE user_id = $1 AND fingerprint = $2 AND is_revoked = $3"#,
    )
    .bind(user_id)
    .bind(fingerprint)
    .bind(false)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("launch device: {e}")))?;
    let Some((device_id, platform)) = device else {
        return Ok(ClaimedWork {
            launches: Vec::new(),
            handoff: None,
        });
    };
    let can_launch = platform == "desktop" || platform == "android";

    let since = Utc::now() - ChronoDuration::seconds(120);
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("launch tx: {e}")))?;
    let claimed: Vec<(Uuid,)> = if can_launch {
        sqlx::query_as(
        r#"INSERT INTO client_launch_acks (launch_id, device_id, acked_at)
           SELECT l.id, $3, $4
           FROM client_launches l
           WHERE l.user_id = $1 AND l.created_at >= $2
             AND (l.device_id IS NULL OR l.device_id = $3)
             AND NOT EXISTS (
               SELECT 1 FROM client_launch_acks a
               WHERE a.launch_id = l.id AND a.device_id = $3
             )
           ORDER BY l.created_at ASC
           LIMIT 3
           ON CONFLICT (launch_id, device_id) DO NOTHING
           RETURNING launch_id"#,
    )
    .bind(user_id)
    .bind(since)
    .bind(device_id)
    .bind(Utc::now())
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("claim launch: {e}")))?
    } else {
        Vec::new()
    };

    let mut out = Vec::with_capacity(claimed.len());
    for (id,) in claimed {
        let (app,): (String,) = sqlx::query_as("SELECT app FROM client_launches WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| AppError::internal(format!("read launch: {e}")))?;
        out.push(ClaimedLaunch { id, app });
    }
    let notes: Vec<(Uuid,)> = sqlx::query_as(
        r#"INSERT INTO device_handoff_acks (handoff_id, device_id, acked_at)
           SELECT h.id, $3, $4
           FROM device_handoffs h
           WHERE h.user_id = $1 AND h.device_id = $3 AND h.created_at >= $2
             AND NOT EXISTS (
               SELECT 1 FROM device_handoff_acks a
               WHERE a.handoff_id = h.id AND a.device_id = $3
             )
           ORDER BY h.created_at ASC
           LIMIT 3
           ON CONFLICT (handoff_id, device_id) DO NOTHING
           RETURNING handoff_id"#,
    )
    .bind(user_id)
    .bind(since)
    .bind(device_id)
    .bind(Utc::now())
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("claim handoff: {e}")))?;
    let mut note_text = Vec::new();
    for (id,) in notes {
        let (content,): (String,) = sqlx::query_as("SELECT content FROM device_handoffs WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| AppError::internal(format!("read handoff: {e}")))?;
        note_text.push(content);
    }
    let handoff = if note_text.is_empty() {
        None
    } else {
        Some(note_text.join("\n\n"))
    };
    sqlx::query("UPDATE devices SET last_seen_at = $2 WHERE id = $1")
        .bind(device_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("touch device: {e}")))?;
    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("launch commit: {e}")))?;
    Ok(ClaimedWork {
        launches: out,
        handoff,
    })
}

pub async fn open_on_vm(app: &str, on_host: bool) -> Result<(), String> {
    if !on_host {
        return Err("API nie działa na maszynie OCI.".into());
    }
    let target = normalize_vm_app(app).map_err(|_| "Niedozwolona nazwa aplikacji.".to_string())?;
    #[cfg(windows)]
    {
        let _ = target;
        return Err("API nie działa na maszynie OCI.".into());
    }
    #[cfg(not(windows))]
    {
        spawn_on_vm(&target).await
    }
}

pub fn normalize_vm_app(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim().trim_matches('"').trim();
    if trimmed.is_empty() || trimmed.chars().count() > 240 || trimmed.chars().any(char::is_control) {
        return Err("invalid app");
    }
    if is_http_url(trimmed) {
        return Ok(trimmed.to_string());
    }
    if trimmed.contains('\\') || trimmed.contains(':') {
        return Err("invalid app");
    }
    if looks_like_path(trimmed) {
        if !trimmed.starts_with('/') || trimmed.starts_with("//") {
            return Err("invalid app");
        }
        return normalize_path(trimmed);
    }
    if is_bare_name(trimmed) {
        return Ok(trimmed.to_string());
    }
    Err("invalid app")
}

fn alias(lower: &str) -> Option<&'static str> {
    Some(match lower {
        "notatnik" => "notepad",
        "kalkulator" | "calculator" => "calc",
        "eksplorator" => "explorer",
        "chrome" | "google chrome" => "chrome",
        "edge" | "microsoft edge" => "msedge",
        "firefox" | "mozilla firefox" => "firefox",
        "vscode" | "visual studio code" => "code",
        "spotify" => "spotify",
        "paint" => "mspaint",
        "ustawienia" | "settings" | "ms-settings" => "ms-settings:",
        "terminal" | "windows terminal" => "wt",
        _ => return None,
    })
}

fn is_settings_uri(lower: &str) -> bool {
    let Some(rest) = lower.strip_prefix("ms-settings:") else {
        return false;
    };
    rest.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn is_http_url(raw: &str) -> bool {
    let Some(rest) = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
    else {
        return false;
    };
    if rest.is_empty() || rest.starts_with('/') {
        return false;
    }
    rest.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '.' | '-' | '_' | '~' | ':' | '/' | '?' | '&' | '=' | '%' | '#' | '+' | '@'
            )
    })
}

fn is_bare_name(raw: &str) -> bool {
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
        && raw.len() <= 64
}

fn looks_like_path(raw: &str) -> bool {
    raw.contains('\\') || raw.contains('/')
}

fn normalize_path(raw: &str) -> Result<String, &'static str> {
    if raw
        .chars()
        .any(|c| matches!(c, '&' | '|' | ';' | '`' | '$' | '<' | '>' | '"' | '\''))
    {
        return Err("invalid app");
    }
    if !is_local_absolute(raw) {
        return Err("invalid app");
    }
    let path = Path::new(raw);
    if path.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err("invalid app");
    }
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "bat" | "cmd" | "ps1" | "vbs" | "js" | "msi" | "scr" | "com" | "jar" | "py" | "sh"
    ) {
        return Err("invalid app");
    }
    if ext == "exe" || ext == "lnk" || (ext.is_empty() && raw.starts_with('/')) {
        return Ok(raw.to_string());
    }
    Err("invalid app")
}

fn is_local_absolute(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        return true;
    }
    raw.starts_with('/') && !raw.starts_with("//")
}

fn tidy(text: &str) -> String {
    let mut out = String::new();
    let mut blank = false;
    let mut started = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            if started {
                blank = true;
            }
            continue;
        }
        if started {
            out.push('\n');
            if blank {
                out.push('\n');
            }
        }
        started = true;
        blank = false;
        out.push_str(line.trim());
    }
    out
}

#[cfg(not(windows))]
async fn spawn_on_vm(target: &str) -> Result<(), String> {
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::process::Command;

    let mut command = Command::new("nsenter");
    command.args(["-t", "1", "-m", "-u", "-i", "-n", "-p", "--"]);
    if target.starts_with("http://") || target.starts_with("https://") {
        command.arg("xdg-open").arg(target);
    } else {
        command.arg(target);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| "Nie udało się uruchomić aplikacji na OCI.".to_string())?;
    match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(_)) | Ok(Err(_)) => Err("Nie udało się uruchomić aplikacji na OCI.".into()),
        Err(_) => {
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        choose_handoff, is_windows_only, normalize_app, normalize_vm_app, resolve_handoff,
        take_launches, windows_desktop, ConnectedDevice, PlannedLaunch,
    };

    fn sample(name: &str, platform: &str, system: &str, fingerprint: &str) -> ConnectedDevice {
        ConnectedDevice {
            id: uuid::Uuid::nil(),
            name: name.into(),
            platform: platform.into(),
            system_name: system.into(),
            fingerprint: fingerprint.into(),
        }
    }

    fn client(app: &str) -> PlannedLaunch {
        PlannedLaunch {
            app: app.into(),
            on_vm: false,
        }
    }

    fn vm(app: &str) -> PlannedLaunch {
        PlannedLaunch {
            app: app.into(),
            on_vm: true,
        }
    }

    #[test]
    fn marker_becomes_an_app_and_leaves_the_sentence() {
        let plan = take_launches("Otwieram notatnik.\n[[launch:notatnik]]\n");
        assert_eq!(plan.text, "Otwieram notatnik.");
        assert_eq!(plan.launches, [client("notepad")]);
        assert!(plan.handoff.is_none());
    }

    #[test]
    fn handoff_marker_names_a_device_and_windows_apps_stay_local() {
        let plan = take_launches("Słucham na komputerze.\n[[handoff:pc-1]]\n[[launch:notatnik]]");
        assert_eq!(plan.text, "Słucham na komputerze.");
        assert_eq!(plan.handoff.as_deref(), Some("pc-1"));
        assert_eq!(plan.launches, [client("notepad")]);
        assert!(is_windows_only("notepad"));
        assert!(is_windows_only(r"C:\Tools\App.exe"));
        assert!(!is_windows_only("firefox"));
        assert!(!is_windows_only("https://example.com"));
        let devices = vec![
            sample("desk", "desktop", "Windows", "pc-1"),
            sample("phone", "android", "Android", "android-1"),
        ];
        assert_eq!(resolve_handoff(&devices, "pc-1").unwrap().name, "desk");
        assert_eq!(windows_desktop(&devices).unwrap().name, "desk");
    }

    #[test]
    fn jarvis_moves_work_the_sender_cannot_finish() {
        let devices = vec![
            sample("browser", "web", "Windows", "web-1"),
            sample("desk", "desktop", "Windows", "pc-1"),
            sample("phone", "android", "Android", "android-1"),
        ];
        let browser = devices.iter().find(|device| device.platform == "web");
        let desk = devices.iter().find(|device| device.platform == "desktop");
        let notepad = choose_handoff(&devices, browser, None, &[client("notepad")]);
        assert_eq!(notepad.target.unwrap().name, "desk");
        assert!(notepad.windows_needed);
        let firefox = choose_handoff(&devices, browser, None, &[client("firefox")]);
        assert_eq!(firefox.target.unwrap().name, "desk");
        let question = choose_handoff(&devices, browser, None, &[]);
        assert!(question.target.is_none());
        let already_there = choose_handoff(&devices, desk, None, &[client("notepad")]);
        assert_eq!(already_there.target.unwrap().fingerprint, "pc-1");
        let asked = choose_handoff(&devices, desk, Some("android-1"), &[client("firefox")]);
        assert_eq!(asked.target.unwrap().name, "phone");
        let missing = choose_handoff(&devices, browser, Some("no-such"), &[]);
        assert!(missing.unresolved);
        assert!(missing.target.is_none());
    }

    #[test]
    fn vm_marker_is_separate_from_the_client() {
        let plan = take_launches("[[launch:firefox]]\n[[launch-vm:firefox]]");
        assert_eq!(plan.launches, [client("firefox"), vm("firefox")]);
        assert!(normalize_vm_app("notepad").is_ok());
        assert!(normalize_vm_app(r"C:\Windows\notepad.exe").is_err());
        assert!(normalize_vm_app("ms-settings:").is_err());
    }

    #[test]
    fn shell_syntax_is_not_an_app() {
        let plan = take_launches("[[launch:notepad & calc]]");
        assert!(plan.text.is_empty());
        assert!(plan.launches.is_empty());
        assert!(normalize_app("cmd /c calc").is_err());
        assert!(normalize_app("javascript:alert(1)").is_err());
        assert!(normalize_app("..\\..\\Windows\\System32\\cmd.exe").is_err());
        assert!(normalize_app("C:\\Tools\\run.bat").is_err());
        assert!(normalize_app("C:\\Tools\\run.ps1").is_err());
    }

    #[test]
    fn urls_and_exe_paths_pass() {
        assert_eq!(
            normalize_app("https://example.com/a?b=1&c=2").unwrap(),
            "https://example.com/a?b=1&c=2"
        );
        assert_eq!(
            normalize_app(r"C:\Program Files\App\App.exe").unwrap(),
            r"C:\Program Files\App\App.exe"
        );
        assert_eq!(normalize_app("ms-settings:apps").unwrap(), "ms-settings:apps");
    }

    #[test]
    fn fourth_launch_is_ignored() {
        let reply = "[[launch:notepad]][[launch:calc]][[launch:explorer]][[launch:mspaint]]";
        let plan = take_launches(reply);
        assert_eq!(
            plan.launches,
            [client("notepad"), client("calc"), client("explorer")]
        );
    }
}
