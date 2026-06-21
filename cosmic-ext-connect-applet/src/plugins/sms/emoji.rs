//! Emoji data and categories for the emoji picker.
// #[allow(dead_code)] = Placeholder for code that will be used once features are fully integrated

#![allow(dead_code)]

/// True if `ch` falls in a Unicode block commonly used for emoji, or is a
/// variation selector / zero-width joiner / keycap combiner used to build
/// emoji sequences. Deliberately broader than the picker's own curated
/// list above — messages can contain any emoji a phone sends, not just the
/// ~500 in our picker grid. Used to detect emoji characters inside
/// arbitrary message text (chat bubbles, conversation previews) so they
/// can be rendered on a font that actually has color glyphs for them,
/// without forcing the whole string (words included) onto an emoji-only
/// font.
pub fn is_emoji_char(ch: char) -> bool {
    matches!(ch as u32,
        0x2300..=0x23FF     // Misc Technical (⌚ ⌛ etc.)
        | 0x2600..=0x27BF   // Misc Symbols + Dingbats
        | 0x2B00..=0x2BFF   // Misc Symbols and Arrows (⭐ etc.)
        | 0x1F000..=0x1FFFF // Mahjong/Dominoes/Cards, Emoticons, Transport,
                            // Supplemental Symbols, Extended-A, flags, etc.
        | 0xFE0F            // Variation Selector-16 (forces emoji presentation)
        | 0x200D            // Zero Width Joiner (combines emoji sequences)
        | 0x20E3            // Combining Enclosing Keycap
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmojiCategory {
    Smileys,
    People,
    Animals,
    Food,
    Travel,
    Activities,
    Objects,
    Symbols,
}

impl EmojiCategory {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Smileys,
            Self::People,
            Self::Animals,
            Self::Food,
            Self::Travel,
            Self::Activities,
            Self::Objects,
            Self::Symbols,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Smileys => "😊",
            Self::People => "👤",
            Self::Animals => "🐶",
            Self::Food => "🍕",
            Self::Travel => "✈️",
            Self::Activities => "⚽",
            Self::Objects => "💡",
            Self::Symbols => "❤️",
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Smileys => fl!("emoji-smileys"),
            Self::People => fl!("emoji-people"),
            Self::Animals => fl!("emoji-animals"),
            Self::Food => fl!("emoji-food"),
            Self::Travel => fl!("emoji-travel"),
            Self::Activities => fl!("emoji-activities"),
            Self::Objects => fl!("emoji-objects"),
            Self::Symbols => fl!("emoji-symbols"),
        }
    }

    pub fn emojis(&self) -> Vec<&'static str> {
        match self {
            Self::Smileys => vec![
                "😀", "😃", "😄", "😁", "😆", "😅", "🤣", "😂", "🙂", "🙃", "😉", "😊", "😇", "🥰",
                "😍", "🤩", "😘", "😗", "😚", "😙", "😋", "😛", "😜", "🤪", "😝", "🤑", "🤗", "🤭",
                "🤫", "🤔", "🤐", "🤨", "😐", "😑", "😶", "😏", "😒", "🙄", "😬", "🤥", "😌", "😔",
                "😪", "🤤", "😴", "😷", "🤒", "🤕", "🤢", "🤮", "🤧", "🥵", "🥶", "😵", "🤯", "🤠",
                "🥳", "😎", "🤓", "🧐", "😕", "😟", "🙁", "☹️",
            ],
            Self::People => vec![
                "👋", "🤚", "🖐️", "✋", "🖖", "👌", "🤏", "✌️", "🤞", "🤟", "🤘", "🤙", "👈", "👉",
                "👆", "🖕", "👇", "☝️", "👍", "👎", "✊", "👊", "🤛", "🤜", "👏", "🙌", "👐", "🤲",
                "🤝", "🙏", "✍️", "💅", "🤳", "💪", "🦾", "🦿", "🦵", "🦶", "👂", "🦻", "👃", "🧠",
                "🦷", "🦴", "👀", "👁️", "👅", "👄", "👶", "🧒", "👦", "👧", "🧑", "👨", "👩", "🧓",
                "👴", "👵", "🙍", "🙎", "🙅", "🙆", "💁", "🙋",
            ],
            Self::Animals => vec![
                "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐨", "🐯", "🦁", "🐮", "🐷", "🐽",
                "🐸", "🐵", "🙈", "🙉", "🙊", "🐒", "🐔", "🐧", "🐦", "🐤", "🐣", "🐥", "🦆", "🦅",
                "🦉", "🦇", "🐺", "🐗", "🐴", "🦄", "🐝", "🐛", "🦋", "🐌", "🐞", "🐜", "🦟", "🦗",
                "🕷️", "🕸️", "🦂", "🐢", "🐍", "🦎", "🦖", "🦕", "🐙", "🦑", "🦐", "🦞", "🦀", "🐡",
                "🐠", "🐟", "🐬", "🐳", "🐋", "🦈", "🐊", "🐅",
            ],
            Self::Food => vec![
                "🍏", "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🍈", "🍒", "🍑", "🥭", "🍍",
                "🥥", "🥝", "🍅", "🍆", "🥑", "🥦", "🥬", "🥒", "🌶️", "🌽", "🥕", "🥔", "🍠", "🥐",
                "🥯", "🍞", "🥖", "🥨", "🧀", "🥚", "🍳", "🥞", "🥓", "🥩", "🍗", "🍖", "🌭", "🍔",
                "🍟", "🍕", "🥪", "🥙", "🌮", "🌯", "🥗", "🥘", "🥫", "🍝", "🍜", "🍲", "🍛", "🍣",
                "🍱", "🥟", "🍤", "🍙", "🍚", "🍘", "🍥", "🥠",
            ],
            Self::Travel => vec![
                "🚗", "🚕", "🚙", "🚌", "🚎", "🏎️", "🚓", "🚑", "🚒", "🚐", "🚚", "🚛", "🚜", "🛴",
                "🚲", "🛵", "🏍️", "🛺", "🚨", "🚔", "🚍", "🚘", "🚖", "🚡", "🚠", "🚟", "🚃", "🚋",
                "🚞", "🚝", "🚄", "🚅", "🚈", "🚂", "🚆", "🚇", "🚊", "🚉", "✈️", "🛫", "🛬", "🛩️",
                "💺", "🛰️", "🚁", "🛶", "⛵", "🚤", "🛳️", "⛴️", "🛥️", "🚢", "⚓", "⛽", "🚧", "🚦",
                "🚥", "🗺️", "🗿", "🗽", "🗼", "🏰", "🏯", "🏟️",
            ],
            Self::Activities => vec![
                "⚽", "🏀", "🏈", "⚾", "🥎", "🎾", "🏐", "🏉", "🥏", "🎱", "🪀", "🏓", "🏸", "🏒",
                "🏑", "🥍", "🏏", "🥅", "⛳", "🪁", "🏹", "🎣", "🤿", "🥊", "🥋", "🎽", "🛹", "🛷",
                "⛸️", "🥌", "🎿", "⛷️", "🏂", "🪂", "🏋️", "🤼", "🤸", "🤺", "⛹️", "🤾", "🏌️", "🏇",
                "🧘", "🏄", "🏊", "🤽", "🚣", "🧗", "🚴", "🚵", "🎪", "🎭", "🎨", "🎬", "🎤", "🎧",
                "🎼", "🎹", "🥁", "🎷", "🎺", "🎸", "🪕", "🎻",
            ],
            Self::Objects => vec![
                "⌚", "📱", "📲", "💻", "⌨️", "🖥️", "🖨️", "🖱️", "🖲️", "🕹️", "🗜️", "💽", "💾", "💿",
                "📀", "📼", "📷", "📸", "📹", "🎥", "📽️", "🎞️", "📞", "☎️", "📟", "📠", "📺", "📻",
                "🎙️", "🎚️", "🎛️", "⏱️", "⏲️", "⏰", "🕰️", "⌛", "⏳", "📡", "🔋", "🔌", "💡", "🔦",
                "🕯️", "🧯", "🛢️", "💸", "💵", "💴", "💶", "💷", "💰", "💳", "💎", "⚖️", "🔧", "🔨",
                "⚒️", "🛠️", "⛏️", "🔩", "⚙️", "⛓️", "🔫", "💣",
            ],
            Self::Symbols => vec![
                "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔", "❣️", "💕", "💞", "💓",
                "💗", "💖", "💘", "💝", "💟", "☮️", "✝️", "☪️", "🕉️", "☸️", "✡️", "🔯", "🕎", "☯️",
                "☦️", "🛐", "⛎", "♈", "♉", "♊", "♋", "♌", "♍", "♎", "♏", "♐", "♑", "♒",
                "♓", "🆔", "⚛️", "🉑", "☢️", "☣️", "📴", "📳", "🈶", "🈚", "🈸", "🈺", "🈷️", "✴️",
                "🆚", "💮", "🉐", "㊙️", "㊗️", "🈴", "🈵", "🈹",
            ],
        }
    }
}
