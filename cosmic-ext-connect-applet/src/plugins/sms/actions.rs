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
    #[allow(dead_code)]
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
}

/// Actions exposed via the window's menu bar. Kept as a separate type
/// (rather than reusing `SmsMessage` directly) because `menu::Item` needs
/// `Copy + Eq + Hash` for key-bind lookups, which `SmsMessage` can't
/// provide since it carries `String`/`Vec` payloads on other variants.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SmsMenuAction {
    NewConversation,
    CloseWindow,
}

impl cosmic::widget::menu::action::MenuAction for SmsMenuAction {
    type Message = SmsMessage;

    fn message(&self) -> Self::Message {
        match self {
            SmsMenuAction::NewConversation => SmsMessage::OpenNewChatDialog,
            SmsMenuAction::CloseWindow => SmsMessage::CloseWindow,
        }
    }
}
