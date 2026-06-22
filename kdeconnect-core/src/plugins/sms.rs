use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::event::ConnectionEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsMessages {
    pub messages: Vec<SmsMessage>,
    pub version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsMessage {
    #[serde(rename = "_id")]
    pub id: i64,
    #[serde(default)]
    pub addresses: Vec<SmsAddress>,
    #[serde(default)]
    pub attachments: Vec<SmsAttachment>,
    #[serde(default)]
    pub body: String,
    pub date: i64,
    #[serde(rename = "type", default)]
    pub message_type: i32,
    #[serde(default)]
    pub read: i32,
    pub thread_id: i64,
    #[serde(default)]
    pub sub_id: Option<i32>,
    #[serde(default)]
    pub event: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsAddress {
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsAttachment {
    pub part_id: i64,
    pub mime_type: String,
    pub encoded_thumbnail: Option<String>,
    pub unique_identifier: Option<String>,
}

impl SmsMessages {
    pub async fn received_packet(&self, tx: mpsc::UnboundedSender<ConnectionEvent>) {
        let event = ConnectionEvent::SmsMessages(self.clone());
        let _ = tx.send(event);
    }
}

/// True if any conversation in `messages_json` has a message newer than
/// what's recorded as "seen" for its thread in `last_seen` — or, for a
/// thread with no entry in `last_seen` at all (never opened in any
/// session), true if the phone itself reports any message in it unread.
///
/// Shared by both the SMS window (which also tracks read state live,
/// in-memory, while a thread is open) and the panel applet (which only
/// needs this one summary bool for its unread badge) so the grouping
/// logic isn't duplicated between the two processes.
pub fn has_unread(
    messages_json: &str,
    last_seen: &std::collections::HashMap<String, i64>,
) -> bool {
    let Ok(data) = serde_json::from_str::<SmsMessages>(messages_json) else {
        return false;
    };

    let mut latest_by_thread: std::collections::HashMap<i64, (i64, bool)> =
        std::collections::HashMap::new();
    for msg in &data.messages {
        let entry = latest_by_thread.entry(msg.thread_id).or_insert((msg.date, msg.read == 0));
        if msg.date > entry.0 {
            *entry = (msg.date, msg.read == 0);
        } else if msg.date == entry.0 && msg.read == 0 {
            entry.1 = true;
        }
    }

    latest_by_thread.into_iter().any(|(thread_id, (date, phone_unread))| {
        match last_seen.get(&thread_id.to_string()) {
            Some(&seen_at) => date > seen_at,
            None => phone_unread,
        }
    })
}
