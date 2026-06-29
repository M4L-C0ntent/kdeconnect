//! The `SmsMessage` action/event enum for the SMS window's Elm-style
//! update loop. Named "actions" rather than "messages" since this is a
//! UI-framework concept (iced/cosmic calls it a "Message") distinct from
//! an actual SMS message — having both in the same file under that name
//! was the source of a previous mix-up.

use std::collections::HashMap;

use super::emoji::EmojiCategory;
use super::models::{Conversation, ProtocolEvent};

/// All possible messages that the SMS window can receive and process.
#[derive(Clone, Debug)]
pub enum SmsMessage {
    LoadConversations,
    #[allow(dead_code)]
    ConversationsLoaded(Vec<Conversation>),
    #[allow(dead_code)]
    ContactsLoaded(HashMap<String, String>),
    SelectThread(String),
    UpdateInput(String),
    UpdateSearch(String),
    SendMessage,
    RefreshThread,
    #[allow(dead_code)]
    CloseWindow,
    ProtocolEventReceived(ProtocolEvent),
    OpenNewChatDialog,
    CloseNewChatDialog,
    UpdateNewChatPhone(String),
    SelectContactForNewChat(String, String),
    CreateNewChat,

    // Emoji picker
    ToggleEmojiPicker,
    SelectEmojiCategory(EmojiCategory),
    InsertEmoji(String),

    /// Opens the confirmation dialog for deleting (hiding) a conversation.
    RequestDeleteConversation(String),
    /// Closes the confirmation dialog without deleting anything.
    CancelDeleteConversation,
    /// Hides the pending conversation from this device's view going
    /// forward. Local-only — the SMS protocol has no delete packet, so
    /// this never touches the phone's actual messages or conversation.
    ConfirmDeleteConversation,

    /// User tapped a thumbnail that hasn't been fully downloaded yet.
    RequestFullAttachment { part_id: i64, unique_identifier: String },
    /// A full-resolution attachment finished downloading. Payload is
    /// (filename/unique_identifier, saved path) — see
    /// `kdeconnect_dbus_client::ServiceEvent::SmsAttachmentReceived`.
    AttachmentReceived(String, std::path::PathBuf),
    /// User wants to open a downloaded attachment in its default external
    /// app (used for video, which iced can't render inline).
    OpenAttachment(std::path::PathBuf),

    /// Opens the native file picker for staging an outgoing attachment.
    PickAttachment,
    /// Files chosen from the picker, appended to `pending_attachments`.
    AttachmentsPicked(Vec<String>),
    /// Removes one staged attachment by index before sending.
    RemovePendingAttachment(usize),
}
