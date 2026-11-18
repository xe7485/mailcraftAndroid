//! Small, context-free helpers shared by foreground and scheduled notifications.

const MAX_TARGET_BYTES: usize = 1024;

pub(super) fn bounded_text(text: &str) -> String {
    const MAX_BYTES: usize = 512;
    let mut bounded = String::with_capacity(text.len().min(MAX_BYTES));
    for character in text.chars() {
        let bidi_control = matches!(character as u32, 0x061C | 0x200E..=0x200F | 0x202A..=0x202E | 0x2066..=0x2069);
        let character = if character.is_control() || bidi_control { ' ' } else { character };
        if bounded.len().saturating_add(character.len_utf8()) > MAX_BYTES {
            break;
        }
        bounded.push(character);
    }
    bounded.trim().to_owned()
}

pub(super) fn reminder_identity(account: &str, uid: &str, occurrence: i64, due: i64, task: bool) -> String {
    format!("{}:{account}{}:{uid}:{occurrence}:{due}:{}", account.len(), uid.len(), u8::from(task))
}

pub(super) fn safe_target_id(value: &str) -> bool {
    value.len() <= MAX_TARGET_BYTES && !value.chars().any(char::is_control)
}
