//! UI view implementations for the SMS window.

use cosmic::widget::button::Catalog;
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget;

use super::actions::SmsMessage;
use super::app::SmsWindow;
use super::emoji::{EmojiCategory, is_emoji_char};
use super::models::Conversation;
use super::utils::{format_timestamp, normalize_phone_number, phone_numbers_match};

/// Max characters shown in the conversation-list preview before truncating
/// with an ellipsis, so every row takes up the same amount of space
/// regardless of how long the underlying message actually is.
const PREVIEW_MAX_CHARS: usize = 50;

/// Truncates by character count (not bytes, so multi-byte emoji aren't cut
/// mid-codepoint) and appends an ellipsis if anything was cut. Doesn't try
/// to avoid splitting multi-codepoint emoji sequences (e.g. ZWJ-joined
/// family emoji) right at the boundary — a rare, low-stakes cosmetic edge
/// case for a preview string, not worth pulling in a grapheme-segmentation
/// dependency for.
fn truncate_preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut truncated: String = s.chars().take(max_chars).collect();
    truncated.push('…');
    truncated
}

/// Renders text as one paragraph made of spans, switching to `EMOJI_FONT`
/// only for characters classified as emoji so they get a font with color
/// glyphs without forcing the surrounding words onto an emoji-only font
/// (which has no Latin glyphs at all). Built on iced's rich-text/span API
/// instead of a Row of separate Text widgets, so it wraps as a single
/// paragraph rather than overflowing on long messages.
///
/// Takes `s` as a plain borrow with its own short-lived scope and copies
/// each span's text into an owned `String` rather than slicing `s`
/// directly — negligible cost at chat-message length, and it means the
/// caller can pass something built on the fly (e.g. a truncated preview)
/// without that temporary having to outlive the returned `Element`.
fn mixed_emoji_text<'a, M: 'a>(s: &str, size: u16) -> Element<'a, M> {
    use cosmic::iced::widget::{rich_text, span};
    use cosmic::iced::widget::text::Span;

    let mut spans: Vec<Span<'a, (), cosmic::iced::Font>> = Vec::new();
    let mut run_start = 0;
    let mut run_is_emoji = false;
    let mut first_run = true;

    let push_run = |spans: &mut Vec<Span<'a, (), cosmic::iced::Font>>, start: usize, end: usize, is_emoji: bool| {
        if start == end {
            return;
        }
        let sp = span(s[start..end].to_string());
        spans.push(if is_emoji { sp.font(EMOJI_FONT) } else { sp });
    };

    for (i, ch) in s.char_indices() {
        let is_emoji = is_emoji_char(ch);
        if first_run {
            run_is_emoji = is_emoji;
            first_run = false;
        } else if is_emoji != run_is_emoji {
            push_run(&mut spans, run_start, i, run_is_emoji);
            run_start = i;
            run_is_emoji = is_emoji;
        }
    }
    push_run(&mut spans, run_start, s.len(), run_is_emoji);

    rich_text::<(), M, cosmic::Theme, cosmic::Renderer>(spans)
        .size(size as f32)
        .into()
}

/// Stable ID for the conversations list scrollable, used to scroll it programmatically.
pub static CONVERSATIONS_SCROLLABLE_ID: std::sync::LazyLock<cosmic::widget::Id> =
    std::sync::LazyLock::new(cosmic::widget::Id::unique);

/// Main view - conversations list + thread view
pub fn view_main(app: &SmsWindow) -> Element<'_, SmsMessage> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    widget::Row::new()
        .spacing(0)
        .push(view_conversations_list(app, &spacing))
        .push(
            widget::container(widget::divider::vertical::default())
                .height(Length::Fill)
                .padding([0, spacing.space_xxs]),
        )
        .push(view_thread_panel(app, &spacing))
        .into()
}

/// New chat dialog view
pub fn view_new_chat_dialog(app: &SmsWindow) -> Element<'_, SmsMessage> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    // Left column: title, input, actions
    let left = widget::Column::new()
        .spacing(spacing.space_m)
        .padding(spacing.space_l)
        .push(
            widget::text(fl!("sms-new-chat-title"))
                .size(20)
                .font(cosmic::font::bold()),
        )
        .push(
            widget::Column::new()
                .spacing(spacing.space_xs)
                .push(widget::text(fl!("sms-new-chat-prompt")).size(14))
                .push(
                    widget::text_input(
                        "e.g., +1-555-123-4567 or John Doe",
                        &app.new_chat_phone_input,
                    )
                    .on_input(SmsMessage::UpdateNewChatPhone)
                    .width(Length::Fill),
                ),
        )
        .push(view_new_chat_actions(app, &spacing))
        .width(Length::Fixed(340.0));

    // Right column: contacts list
    let right = widget::Column::new()
        .spacing(spacing.space_s)
        .padding(spacing.space_l)
        .push(widget::text(fl!("sms-new-chat-contacts")).size(16).font(cosmic::font::bold()))
        .push(view_contacts_list(app, &spacing))
        .width(Length::Fill);

    let content = widget::Row::new()
        .push(left)
        .push(widget::divider::vertical::default())
        .push(right)
        .height(Length::Fixed(500.0));

    widget::container(content)
        .class(cosmic::theme::Container::Card)
        .width(Length::Fill)
        .into()
}

fn view_new_chat_actions<'a>(
    app: &'a SmsWindow,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let start_button_enabled = !app.new_chat_phone_input.trim().is_empty();

    widget::Row::new()
        .spacing(spacing.space_xs)
        .push(widget::button::standard(fl!("sms-new-chat-cancel")).on_press(SmsMessage::CloseNewChatDialog))
        .push(widget::space::horizontal())
        .push(if start_button_enabled {
            widget::button::suggested(fl!("sms-new-chat-start")).on_press(SmsMessage::CreateNewChat)
        } else {
            widget::button::suggested(fl!("sms-new-chat-start"))
        })
        .into()
}

fn view_contacts_list<'a>(
    app: &'a SmsWindow,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    if app.contacts.is_empty() {
        return widget::text(fl!("sms-new-chat-no-contacts")).size(12).into();
    }

    let mut contacts_list = widget::Column::new().spacing(spacing.space_xxs);
    let filtered_contacts = get_filtered_contacts(app);

    if filtered_contacts.is_empty() {
        contacts_list = contacts_list.push(widget::text(fl!("sms-new-chat-no-matches")).size(12));
    } else {
        for (phone, name) in filtered_contacts.iter() {
            contacts_list = contacts_list.push(
                widget::button::text(format!("{} ({})", name, phone))
                    .on_press(SmsMessage::SelectContactForNewChat(
                        phone.to_string(),
                        name.to_string(),
                    ))
                    .width(Length::Fill),
            );
        }
        contacts_list = contacts_list.push(
            widget::container(
                widget::text(fl!("sms-new-chat-showing", count = (filtered_contacts.len() as i64)))
                .size(11),
            )
            .padding([spacing.space_xs, 0, 0, 0]),
        );
    }

    widget::scrollable(contacts_list)
        .height(Length::Fill)
        .into()
}

fn get_filtered_contacts(app: &SmsWindow) -> Vec<(&String, &String)> {
    let mut sorted_contacts: Vec<_> = app.contacts.iter().collect();
    sorted_contacts.sort_by(|a, b| a.1.cmp(b.1));

    let search_term = app.new_chat_phone_input.trim().to_lowercase();

    if search_term.is_empty() {
        sorted_contacts
    } else {
        sorted_contacts
            .into_iter()
            .filter(|(phone, name)| {
                name.to_lowercase().contains(&search_term)
                    || phone.contains(&search_term)
                    || normalize_phone_number(phone).contains(&normalize_phone_number(&search_term))
            })
            .collect()
    }
}

/// Manually re-syncs conversations from the phone. `LoadConversations` was
/// already fully wired in `update()` but had no UI entry point — everything
/// else updates via the live event stream.
fn view_refresh_button<'a>() -> Element<'a, SmsMessage> {
    widget::button::icon(widget::icon::from_name("view-refresh-symbolic").handle())
        .on_press(SmsMessage::LoadConversations)
        .into()
}

/// Conversations list panel
fn view_conversations_list<'a>(
    app: &'a SmsWindow,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let mut content = widget::Column::new().spacing(spacing.space_xs);

    // Hamburger menu + Start Chat, right-aligned
    content = content.push(
        widget::container(
            widget::Row::new()
                .push(view_refresh_button())
                .push(widget::space::horizontal())
                .push(
                    widget::button::suggested(fl!("sms-new-chat-start"))
                        .on_press(SmsMessage::OpenNewChatDialog)
                        .class(crate::theme::accent_filled_button(app.accent_color)),
                )
                .align_y(Alignment::Center)
                .spacing(spacing.space_xs),
        )
        .padding(spacing.space_s),
    );

    // Search input
    content = content.push(
        widget::search_input(fl!("sms-search-placeholder"), &app.search_query)
            .on_input(SmsMessage::UpdateSearch),
    );
    content = content.push(widget::divider::horizontal::default());

    // Filter conversations
    let mut filtered: Vec<_> = app
        .conversations
        .iter()
        .filter(|c| conversation_matches_search(app, c))
        .collect();

    // Sort by timestamp (most recent first)
    filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    if filtered.is_empty() {
        let msg = if app.search_query.is_empty() {
            fl!("sms-no-conversations")
        } else {
            fl!("sms-no-matching-conversations")
        };

        content = content.push(
            widget::container(widget::text(msg).size(14))
                .width(Length::Fill)
                .padding(spacing.space_xl)
                .center_x(Length::Fill),
        );
    } else {
        let mut list = widget::Column::new().spacing(0);

        for conv in filtered {
            list = list.push(view_conversation_item(app, conv, spacing));
            list = list.push(widget::divider::horizontal::light());
        }

        content = content.push(widget::scrollable(list).height(Length::Fill));
    }

    widget::container(content)
        .width(Length::Fixed(300.0))
        .height(Length::Fill)
        .into()
}

fn conversation_matches_search(app: &SmsWindow, conv: &Conversation) -> bool {
    if app.search_query.is_empty() {
        return true;
    }

    let query = app.search_query.to_lowercase();
    conv.contact_name.to_lowercase().contains(&query)
        || conv.phone_number.contains(&app.search_query)
        || normalize_phone_number(&conv.phone_number)
            .contains(&normalize_phone_number(&app.search_query))
}

fn view_conversation_item<'a>(
    app: &'a SmsWindow,
    conv: &'a Conversation,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let is_selected = app.selected_thread.as_ref() == Some(&conv.thread_id);

    let display_name =
        get_contact_name(app, &conv.phone_number).unwrap_or_else(|| conv.phone_number.clone());

    let accent = app.accent_color;
    let flat_button_class = cosmic::theme::Button::Custom {
        active: Box::new(move |focused, theme| {
            let mut s = theme.active(focused, false, &cosmic::theme::Button::Text);
            s.border_radius = cosmic::iced::Radius::from(0.0);
            s.text_color = Some(accent);
            s
        }),
        hovered: Box::new(move |focused, theme| {
            let mut s = theme.hovered(focused, false, &cosmic::theme::Button::Text);
            s.border_radius = cosmic::iced::Radius::from(0.0);
            s.text_color = Some(accent);
            s
        }),
        disabled: Box::new(|theme| {
            let mut s = theme.disabled(&cosmic::theme::Button::Text);
            s.border_radius = cosmic::iced::Radius::from(0.0);
            s
        }),
        pressed: Box::new(move |focused, theme| {
            let mut s = theme.pressed(focused, false, &cosmic::theme::Button::Text);
            s.border_radius = cosmic::iced::Radius::from(0.0);
            s.text_color = Some(accent);
            s
        }),
    };

    let unread = is_conversation_unread(app, conv);

    let mut name_row = widget::Row::new()
        .push(
            widget::text(display_name)
                .size(14)
                .font(cosmic::font::bold()),
        )
        .spacing(spacing.space_xs);

    if unread {
        name_row = name_row.push(
            widget::container(widget::Space::new().width(Length::Fixed(8.0)).height(Length::Fixed(8.0)))
                .class(cosmic::theme::Container::custom(move |_theme| {
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Background::Color(accent)),
                        border: cosmic::iced::Border {
                            radius: cosmic::iced::Radius::from(4.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                })),
        );
    }

    let button = widget::button::custom(
        widget::Column::new()
            .push(
                name_row
                    .push(widget::space::horizontal())
                    .push(widget::text(format_timestamp(conv.timestamp)).size(11)),
            )
            .push(mixed_emoji_text(&truncate_preview(&conv.last_message, PREVIEW_MAX_CHARS), 12))
            .spacing(spacing.space_xxs)
            .padding(spacing.space_s),
    )
    .class(flat_button_class)
    .on_press(SmsMessage::SelectThread(conv.thread_id.clone()))
    .width(Length::Fill);

    let row_content = if is_selected {
        widget::container(button)
            .class(cosmic::theme::Container::Primary)
            .into()
    } else {
        Element::from(button)
    };

    let delete_button = widget::button::icon(widget::icon::from_name("user-trash-symbolic").handle())
        .on_press(SmsMessage::RequestDeleteConversation(conv.thread_id.clone()));

    widget::Row::new()
        .push(row_content)
        .push(delete_button)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

/// Thread panel (messages + input)
fn view_thread_panel<'a>(
    app: &'a SmsWindow,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let Some(thread_id) = &app.selected_thread else {
        return widget::container(widget::text(fl!("sms-select-conversation")).size(14))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    };

    let Some(conv) = app.conversations.iter().find(|c| c.thread_id == *thread_id) else {
        return widget::container(widget::text(fl!("sms-conversation-not-found")).size(14))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    };

    let mut content = widget::Column::new().spacing(0);

    // Header
    content = content.push(view_thread_header(app, conv, spacing));
    content = content.push(widget::divider::horizontal::default());

    // Messages
    content = content.push(view_messages_list(app, spacing));
    content = content.push(widget::divider::horizontal::default());

    // Input
    content = content.push(view_message_input(app, spacing));

    widget::container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_thread_header<'a>(
    app: &'a SmsWindow,
    conv: &'a Conversation,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let display_name =
        get_contact_name(app, &conv.phone_number).unwrap_or_else(|| conv.phone_number.clone());

    widget::container(
        widget::Column::new()
            .push(
                widget::text(display_name)
                    .size(16)
                    .font(cosmic::font::bold()),
            )
            .push(widget::text(&conv.phone_number).size(12))
            .spacing(spacing.space_xxs)
            .padding(spacing.space_s),
    )
    .class(cosmic::theme::Container::Card)
    .width(Length::Fill)
    .into()
}

fn view_messages_list<'a>(
    app: &'a SmsWindow,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let mut messages_column = widget::Column::new()
        .spacing(spacing.space_m)
        .padding(spacing.space_m);

    if app.messages.is_empty() {
        messages_column = messages_column.push(
            widget::container(
                widget::Column::new()
                    .push(widget::text(fl!("sms-waiting-for-messages")).size(14))
                    .push(
                        widget::text(fl!("sms-messages-will-appear"))
                            .size(12),
                    )
                    .spacing(spacing.space_xs)
                    .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding(spacing.space_xl),
        );
    } else {
        for msg in &app.messages {
            messages_column = messages_column.push(view_message_bubble(app, msg, spacing));
        }
    }

    widget::scrollable(messages_column)
        .height(Length::Fill)
        .direction(cosmic::iced::widget::scrollable::Direction::Vertical(
            cosmic::iced::widget::scrollable::Scrollbar::new()
                .anchor(cosmic::iced::widget::scrollable::Anchor::End),
        ))
        .into()
}

/// Renders one MMS attachment. Three states, and — unlike before — none
/// of them render nothing, since a thumbnail-less attachment used to
/// produce a blank bubble (the bug this replaces):
/// - already downloaded + image → the full-resolution image (loaded by
///   path, so iced caches it instead of us re-reading the file)
/// - already downloaded + video → an "Open" button (iced can't render
///   video inline)
/// - not downloaded yet → the thumbnail preview if the phone sent one,
///   otherwise a generic photo/video placeholder card — either way
///   clickable to request the full file, *if* the phone gave it a
///   `unique_identifier` to request by. Video in particular routinely has
///   no thumbnail at all in this protocol, so the placeholder path is the
///   common case for video, not a rare fallback.
fn view_attachment<'a>(
    attachment: &'a super::models::MessageAttachment,
) -> Option<Element<'a, SmsMessage>> {
    if let Some(path) = &attachment.full_path {
        return Some(if attachment.is_video() {
            widget::button::standard(fl!("sms-open-attachment"))
                .on_press(SmsMessage::OpenAttachment(path.clone()))
                .into()
        } else {
            widget::image(cosmic::widget::image::Handle::from_path(path))
                .width(Length::Fixed(220.0))
                .into()
        });
    }

    let preview: Element<'a, SmsMessage> = match attachment.thumbnail.as_deref() {
        Some(thumbnail) if !thumbnail.is_empty() => {
            widget::image(cosmic::widget::image::Handle::from_bytes(thumbnail.to_vec()))
                .width(Length::Fixed(220.0))
                .into()
        }
        _ => view_attachment_placeholder(attachment),
    };

    Some(match &attachment.unique_identifier {
        Some(uid) => widget::mouse_area(preview)
            .on_press(SmsMessage::RequestFullAttachment {
                part_id: attachment.part_id,
                unique_identifier: uid.clone(),
            })
            .into(),
        None => preview,
    })
}

/// Generic icon + label card shown in place of a thumbnail when the phone
/// didn't send one. Video routinely has no thumbnail in this protocol at
/// all, so this is the normal video appearance, not a rare error state.
fn view_attachment_placeholder<'a>(
    attachment: &super::models::MessageAttachment,
) -> Element<'a, SmsMessage> {
    let (icon_name, label) = if attachment.is_video() {
        ("video-x-generic-symbolic", fl!("sms-attachment-video"))
    } else if attachment.mime_type.starts_with("image/") {
        ("image-x-generic-symbolic", fl!("sms-attachment-photo"))
    } else {
        ("mail-attachment-symbolic", fl!("sms-attachment-generic"))
    };

    widget::container(
        widget::Column::new()
            .push(widget::icon::from_name(icon_name).icon().size(48))
            .push(widget::text(label).size(12))
            .spacing(4)
            .align_x(Alignment::Center),
    )
    .width(Length::Fixed(220.0))
    .padding(16)
    .align_x(Alignment::Center)
    .class(cosmic::theme::Container::List)
    .into()
}

fn view_message_bubble<'a>(
    app: &'a SmsWindow,
    msg: &'a super::models::Message,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let is_sent = msg.is_sent();
    let mut message_content = widget::Column::new().spacing(spacing.space_xxs);

    // Show sender label only for received messages
    if !is_sent {
        let phone_number =
            get_current_conversation_phone(app).unwrap_or_else(|| msg.address.clone());

        let sender_label = get_contact_name(app, &phone_number).unwrap_or(phone_number);

        message_content = message_content.push(
            widget::text(sender_label)
                .size(11)
                .font(cosmic::font::bold()),
        );
    }

    for attachment in &msg.attachments {
        if let Some(element) = view_attachment(attachment) {
            message_content = message_content.push(element);
        }
    }

    message_content = message_content
        .push(mixed_emoji_text(&msg.body, 14))
        .push(widget::text(format_timestamp(msg.date)).size(11))
        .padding(spacing.space_s);

    let message_bubble = if is_sent {
        widget::container(message_content)
            .class(cosmic::theme::Container::Primary)
            .max_width(500.0)
    } else {
        widget::container(message_content)
            .class(cosmic::theme::Container::Secondary)
            .max_width(500.0)
    };

    if is_sent {
        widget::Row::new()
            .push(widget::space::horizontal())
            .push(message_bubble)
            .width(Length::Fill)
            .into()
    } else {
        widget::Row::new()
            .push(message_bubble)
            .width(Length::Fill)
            .into()
    }
}

fn view_message_input<'a>(
    app: &'a SmsWindow,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let message_placeholder = fl!("sms-message-placeholder");

    let input_row = widget::Row::new()
        .push(
            widget::button::icon(widget::icon::from_name("face-smile-symbolic").handle())
                .on_press(SmsMessage::ToggleEmojiPicker),
        )
        .push(
            widget::button::icon(widget::icon::from_name("mail-attachment-symbolic").handle())
                .on_press(SmsMessage::PickAttachment),
        )
        .push(
            widget::text_input(message_placeholder, &app.message_input)
                .on_input(SmsMessage::UpdateInput)
                .on_submit(|_| SmsMessage::SendMessage)
                .padding(spacing.space_s)
                .width(Length::Fill),
        )
        .push(
            widget::button::suggested(fl!("sms-send"))
                .on_press(SmsMessage::SendMessage)
                .class(crate::theme::accent_filled_button(app.accent_color))
        )
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center);

    let mut col = widget::Column::new().spacing(spacing.space_xs);
    if app.show_emoji_picker {
        col = col.push(view_emoji_picker(app, spacing));
    }
    if !app.pending_attachments.is_empty() {
        col = col.push(view_pending_attachments(app, spacing));
    }
    col = col.push(input_row);

    col.padding(spacing.space_s).into()
}

/// Staged outgoing attachments, shown as a row of removable chips above
/// the compose bar. Just the filename — no preview thumbnail, since these
/// are local files we haven't sent yet (not worth a thumbnail decode for
/// something this transient).
fn view_pending_attachments<'a>(
    app: &'a SmsWindow,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let mut row = widget::Row::new().spacing(spacing.space_xs);
    for (index, path) in app.pending_attachments.iter().enumerate() {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());

        row = row.push(
            widget::container(
                widget::Row::new()
                    .push(widget::text(name).size(12))
                    .push(
                        widget::button::icon(widget::icon::from_name("window-close-symbolic").handle())
                            .on_press(SmsMessage::RemovePendingAttachment(index)),
                    )
                    .spacing(spacing.space_xxs)
                    .align_y(Alignment::Center),
            )
            .class(cosmic::theme::Container::Secondary)
            .padding(spacing.space_xxs),
        );
    }
    widget::scrollable(row)
        .direction(cosmic::iced::widget::scrollable::Direction::Horizontal(
            cosmic::iced::widget::scrollable::Scrollbar::new(),
        ))
        .into()
}

/// Emoji picker panel: category tabs + a scrollable grid of emoji for the
/// selected category. Stays open after inserting an emoji so multiple can
/// be picked in a row.
// cosmic-text's font fallback can resolve a handful of codepoints (✈️⚽💡❤️,
// some smileys) to a non-color font that happens to also cover them, instead
// of the color emoji font, which renders them as outlines tinted by the
// button's text color. Pinning the glyph to the color emoji font avoids that.
const EMOJI_FONT: cosmic::iced::Font = cosmic::iced::Font::with_name("Noto Color Emoji");

fn view_emoji_picker<'a>(
    app: &'a SmsWindow,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, SmsMessage> {
    let mut tabs = widget::Row::new()
        .spacing(spacing.space_xxs)
        .width(Length::Fill);
    for category in EmojiCategory::all() {
        let is_active = app.emoji_category == category;
        let tab = widget::button::custom(
            widget::text(category.label())
                .font(EMOJI_FONT)
                .width(Length::Fill)
                .align_x(Alignment::Center),
        )
        .padding(spacing.space_xxs)
        .width(Length::Fill)
        .on_press(SmsMessage::SelectEmojiCategory(category));
        tabs = tabs.push(if is_active {
            tab.class(cosmic::theme::Button::Suggested)
        } else {
            tab.class(cosmic::theme::Button::Text)
        });
    }

    let mut grid = widget::Column::new().spacing(spacing.space_xxs);
    for row_emojis in app.emoji_category.emojis().chunks(8) {
        let mut row = widget::Row::new().spacing(spacing.space_xxs).width(Length::Fill);
        for emoji in row_emojis {
            row = row.push(
                widget::button::custom(
                    widget::text(*emoji)
                        .font(EMOJI_FONT)
                        .width(Length::Fill)
                        .align_x(Alignment::Center),
                )
                .padding(spacing.space_xxs)
                .width(Length::Fill)
                .on_press(SmsMessage::InsertEmoji(emoji.to_string())),
            );
        }
        grid = grid.push(row);
    }

    widget::container(
        widget::Column::new()
            .spacing(spacing.space_xs)
            .padding(spacing.space_s)
            .push(tabs)
            .push(widget::divider::horizontal::light())
            .push(
                widget::scrollable(grid.width(Length::Fill))
                    .width(Length::Fill)
                    .height(Length::Fixed(160.0)),
            ),
    )
    .class(cosmic::theme::Container::Card)
    .width(Length::Fill)
    .into()
}

// Helper functions

fn get_contact_name(app: &SmsWindow, phone_number: &str) -> Option<String> {
    app.contacts
        .iter()
        .find(|(contact_phone, _)| phone_numbers_match(phone_number, contact_phone))
        .map(|(_, name)| name.clone())
}

fn get_current_conversation_phone(app: &SmsWindow) -> Option<String> {
    let thread_id = app.selected_thread.as_ref()?;
    app.conversations
        .iter()
        .find(|c| c.thread_id == *thread_id)
        .map(|c| c.phone_number.clone())
}

/// True if this conversation should show the unread indicator. Once a
/// thread has been opened in this app session, the phone's own read flag
/// is ignored in favor of comparing against the last message timestamp
/// the user actually saw — there's no protocol way to write "read" back
/// to the phone, so mirroring its flag forever would mean the badge never
/// clears just because you read it here. For threads never opened this
/// session, falls back to the phone-reported flag as a reasonable guess.
fn is_conversation_unread(app: &SmsWindow, conv: &Conversation) -> bool {
    match app.last_seen_timestamp.get(&conv.thread_id) {
        Some(&seen_at) => conv.timestamp > seen_at,
        None => conv.unread,
    }
}
