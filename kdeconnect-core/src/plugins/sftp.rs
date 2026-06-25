use std::path::{Path, PathBuf};

use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::info;

/// `kdeconnect.sftp` — login info the phone sends in response to a
/// `kdeconnect.sftp.request { startBrowsing: true }`.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SftpInfo {
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub multi_paths: Option<Vec<String>>,
}

impl SftpInfo {
    /// Mount the phone's SFTP share with `sshfs` (FUSE) and open it in the
    /// file manager — the same mechanism the official kdeconnect-kde daemon
    /// uses. Every command runs via `host_command()`: inside a Flatpak
    /// sandbox a FUSE mount made in-sandbox lives in the sandbox's own
    /// mount namespace and is invisible to the host, so mounting/opening
    /// has to happen as real host processes via `flatpak-spawn --host`.
    /// Outside a sandbox this is just a plain `Command::new`.
    pub async fn browse(&self, device_id: &str) -> anyhow::Result<()> {
        preflight().await?;

        let mount_point = mount_point(device_id);
        let mount_point_str = mount_point.to_string_lossy().into_owned();

        run(host_command("mkdir").args(["-p", &mount_point_str])).await?;

        if is_mounted(&mount_point_str).await {
            info!("[sftp] {} already mounted", mount_point_str);
            return open(&mount_point_str).await;
        }

        let path = self
            .multi_paths
            .as_ref()
            .and_then(|p| p.first())
            .cloned()
            .or_else(|| self.path.clone())
            .unwrap_or_else(|| "/".to_string());

        info!(
            "[sftp] mounting {}@{}:{} -> {}",
            self.user, self.ip, self.port, mount_point_str
        );

        let mut child = host_command("sshfs")
            .arg(format!("{}@{}:{}", self.user, self.ip, path))
            .arg(&mount_point_str)
            .args([
                "-p",
                &self.port.to_string(),
                // Each browse session spins up a brand-new ephemeral SSH
                // host key on the phone, so there's nothing meaningful to
                // pin — host checking would just prompt-and-fail every time.
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "password_stdin",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(format!("{}\n", self.password).as_bytes())
                .await?;
        }

        let output = child.wait_with_output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("sshfs failed: {}", stderr.trim());
        }

        open(&mount_point_str).await
    }
}

/// Specific, actionable reasons `browse()` can't proceed — surfaced to the
/// user as-is, so each variant's message names the actual fix rather than a
/// generic "mount failed".
#[derive(Debug, Clone)]
pub enum BrowsePreflightError {
    /// `sshfs` isn't on the host. A missing-package problem — no sandbox
    /// permission fixes this.
    SshfsMissing,
    /// `flatpak-spawn --host` itself failed. Usually means the
    /// `org.freedesktop.Flatpak` D-Bus name was revoked (e.g. in Flatseal).
    FlatpakSpawnUnavailable,
}

impl std::fmt::Display for BrowsePreflightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SshfsMissing => write!(
                f,
                "sshfs is not installed on this system. Install it (e.g. `sudo apt install sshfs` or `sudo dnf install fuse-sshfs`) to browse device files."
            ),
            Self::FlatpakSpawnUnavailable => write!(
                f,
                "Can't reach the host to mount the device. Check that this app's \"org.freedesktop.Flatpak\" permission hasn't been revoked (e.g. in Flatseal)."
            ),
        }
    }
}

impl std::error::Error for BrowsePreflightError {}

async fn preflight() -> Result<(), BrowsePreflightError> {
    if in_flatpak() {
        let reachable = Command::new("flatpak-spawn")
            .args(["--host", "true"])
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);
        if !reachable {
            return Err(BrowsePreflightError::FlatpakSpawnUnavailable);
        }
    }

    let has_sshfs = host_command("sh")
        .args(["-c", "command -v sshfs"])
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);
    if !has_sshfs {
        return Err(BrowsePreflightError::SshfsMissing);
    }

    Ok(())
}

/// True when running inside a Flatpak sandbox.
fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// Builds a command that runs on the host when sandboxed (via
/// `flatpak-spawn --host`, already permitted by
/// `--talk-name=org.freedesktop.Flatpak` in the manifest) and runs directly
/// otherwise.
fn host_command(program: &str) -> Command {
    if in_flatpak() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(program);
        cmd
    } else {
        Command::new(program)
    }
}

fn mount_point(device_id: &str) -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join("kdeconnect-sftp").join(device_id)
}

/// Reads the host's `/proc/mounts` (via `host_command`, so this is the
/// host's mount table even when sandboxed) rather than depending on the
/// `mountpoint` binary just to answer a question the kernel already tracks.
async fn is_mounted(target: &str) -> bool {
    let output = match host_command("cat").arg("/proc/mounts").output().await {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|l| l.split_whitespace().nth(1) == Some(target))
}

async fn run(cmd: &mut Command) -> anyhow::Result<()> {
    let output = cmd.output().await?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

async fn open(mount_point: &str) -> anyhow::Result<()> {
    host_command("xdg-open").arg(mount_point).spawn()?;
    Ok(())
}
