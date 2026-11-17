use tauri_plugin_notification::LetterTile;

/// Keep in sync with `text-avatar-data-url.ts`.
const TEXT_AVATAR_PREFIX: &str = "data:application/x-dashchat-avatar,TextAvatarData|1|";
/// Keep in sync with `TEXT_AVATAR_TEXT_COLOR` in `Avatar.svelte`.
const TEXT_AVATAR_TEXT_COLOR: &str = "#831843";
const GENERIC_AVATAR_GREY: &str = "#8e8e93";
const WHITE: &str = "#ffffff";

/// What the app draws for a sender without a photo, like Signal: their text
/// avatar, else the initials of their name, else (no text) a person silhouette.
pub(super) fn sender_letter_tile(avatar: Option<&str>, name: Option<&str>) -> LetterTile {
    if let Some((background_color, text)) = avatar.and_then(parse_text_avatar) {
        return LetterTile {
            text: Some(text),
            background_color,
            text_color: TEXT_AVATAR_TEXT_COLOR.to_string(),
        };
    }
    LetterTile {
        text: name
            .map(name_initials)
            .filter(|initials| !initials.is_empty()),
        background_color: GENERIC_AVATAR_GREY.to_string(),
        text_color: WHITE.to_string(),
    }
}

fn parse_text_avatar(avatar: &str) -> Option<(String, String)> {
    let (encoded_color, text) = avatar.strip_prefix(TEXT_AVATAR_PREFIX)?.split_once('|')?;
    let hex = encoded_color.strip_prefix("%23")?;
    let color_is_valid = hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit());
    let text_is_valid = (1..=3).contains(&text.len())
        && text
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    (color_is_valid && text_is_valid).then(|| (format!("#{hex}"), text.to_string()))
}

/// The app shows the first two characters of the name; spaces and punctuation
/// among them draw nothing, so they are dropped.
fn name_initials(name: &str) -> String {
    name.trim()
        .chars()
        .take(2)
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_a_text_avatar_in_its_own_colour() {
        let avatar = "data:application/x-dashchat-avatar,TextAvatarData|1|%23fbcfe8|GC";
        let tile = sender_letter_tile(Some(avatar), Some("Bob"));
        assert_eq!(tile.text.as_deref(), Some("GC"));
        assert_eq!(tile.background_color, "#fbcfe8");
        assert_eq!(tile.text_color, TEXT_AVATAR_TEXT_COLOR);
    }

    #[test]
    fn falls_back_to_the_initials_of_a_malformed_text_avatar() {
        let avatar = "data:application/x-dashchat-avatar,TextAvatarData|1|%23fbcfe8|<b>";
        let tile = sender_letter_tile(Some(avatar), Some("Bob"));
        assert_eq!(tile.text.as_deref(), Some("BO"));
        assert_eq!(tile.background_color, GENERIC_AVATAR_GREY);
    }

    #[test]
    fn takes_the_letters_among_the_first_two_characters() {
        assert_eq!(name_initials(" bob "), "BO");
        assert_eq!(name_initials("J Doe"), "J");
        assert_eq!(name_initials("J.R."), "J");
        assert_eq!(name_initials("مریم"), "مر");
    }

    #[test]
    fn leaves_the_text_out_for_the_person_silhouette() {
        assert_eq!(sender_letter_tile(None, None).text, None);
        assert_eq!(sender_letter_tile(None, Some("..")).text, None);
    }
}
