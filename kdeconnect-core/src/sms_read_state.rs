//! Persists per-device "last seen message" timestamps, keyed by thread ID.
//!
//! The SMS window already tracks this in memory while it's open (it's how
//! the unread dot in its own sidebar works — see SelectThread and
//! MessageReceived in cosmic-ext-connect-applet's plugins/sms/app.rs).
//! This module exists so that state can *also* reach the panel applet,
//! which is a separate process with no other way to know what's been
//! read — there's no protocol packet for "read" at all (see
//! hidden_conversations.rs for the same constraint on delete), so this is
//! the only mechanism available, local-only, same as that module.
//!
//! Stored as a JSON object of thread ID -> timestamp at:
//!   ~/.config/kdeconnect/{device_id}_last_seen_messages.json

use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::CONFIG_DIR;

fn config_path(device_id: &str) -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(CONFIG_DIR)
        .join(format!("{}_last_seen_messages.json", device_id))
}

/// Load the last-seen map for a device. Synchronous and tiny — meant to be
/// called once at startup, same rationale as hidden_conversations::load_hidden.
/// Returns an empty map if no file exists yet.
pub fn load_last_seen(device_id: &str) -> HashMap<String, i64> {
    std::fs::read_to_string(config_path(device_id))
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Persist the last-seen map for a device.
pub async fn save_last_seen(device_id: &str, last_seen: &HashMap<String, i64>) {
    let path = config_path(device_id);
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match serde_json::to_string(last_seen) {
        Ok(json) => {
            if let Err(e) = tokio::fs::write(&path, json).await {
                tracing::warn!("[sms_read_state] failed to save for {}: {}", device_id, e);
            }
        }
        Err(e) => tracing::warn!("[sms_read_state] serialize failed: {}", e),
    }
}
