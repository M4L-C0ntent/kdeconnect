use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Clipboard {
    pub content: String,
    pub timestamp: Option<u64>,
}

impl Clipboard {
    pub async fn received_packet(
        &self,
        event: mpsc::UnboundedSender<crate::event::ConnectionEvent>,
    ) {
        let _ = event.send(crate::event::ConnectionEvent::ClipboardReceived(
            self.content.clone(),
        ));
    }

    pub async fn received_connect_packet(
        &self,
        event: mpsc::UnboundedSender<crate::event::ConnectionEvent>,
    ) {
        let _ = event.send(crate::event::ConnectionEvent::ClipboardConnectReceived {
            content: self.content.clone(),
            timestamp: self.timestamp,
        });
    }
}
