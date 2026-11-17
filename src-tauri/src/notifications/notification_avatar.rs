use dashchat_node::Profile;
use tauri_plugin_notification::LetterTile;

/// Keep in sync with `text-avatar-data-url.ts`.
const TEXT_AVATAR_PREFIX: &str = "data:application/x-dashchat-avatar,TextAvatarData|1|";
/// Keep in sync with `TEXT_AVATAR_TEXT_COLOR` in `Avatar.svelte`.
const TEXT_AVATAR_TEXT_COLOR: &str = "#831843";
const GENERIC_AVATAR_GREY: &str = "#8e8e93";
const WHITE: &str = "#ffffff";

/// The sender's photo as `large_icon_bytes`, or else, like Signal, what the
/// app draws for them as `large_icon_letter_tile`. Without their profile yet,
/// the app draws a silhouette, not the initials of the name in their code.
pub(super) fn notification_avatar(
    profile: Option<Profile>,
) -> (Option<String>, Option<LetterTile>) {
    let Some(profile) = profile else {
        return (None, Some(sender_letter_tile(None, None)));
    };
    match profile.avatar {
        Some(photo) if photo.starts_with("data:image/") => (Some(photo), None),
        avatar => (
            None,
            Some(sender_letter_tile(avatar.as_deref(), Some(&profile.name))),
        ),
    }
}

/// Their text avatar, else the initials of their name, else no text, which
/// the plugin draws as a person silhouette.
fn sender_letter_tile(avatar: Option<&str>, name: Option<&str>) -> LetterTile {
    if let Some((background_color, text)) = avatar.and_then(parse_text_avatar) {
        return LetterTile {
            text: Some(text),
            background_color,
            text_color: TEXT_AVATAR_TEXT_COLOR.to_string(),
        };
    }
    LetterTile {
        text: name.map(name_initials),
        background_color: GENERIC_AVATAR_GREY.to_string(),
        text_color: WHITE.to_string(),
    }
}

fn parse_text_avatar(avatar: &str) -> Option<(String, String)> {
    let mut fields = avatar.strip_prefix(TEXT_AVATAR_PREFIX)?.split('|');
    let hex = fields.next()?.strip_prefix("%23")?;
    let text = fields.next()?;
    let color_is_valid = hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit());
    let text_is_valid = (1..=3).contains(&text.len())
        && text
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    (color_is_valid && text_is_valid).then(|| (format!("#{hex}"), text.to_string()))
}

/// Mirrors the app's avatar, which shows `name.slice(0, 2)` uppercased.
fn name_initials(name: &str) -> String {
    name.chars().take(2).collect::<String>().to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_photo() {
        let photo = "data:image/jpeg;base64,AAAA".to_string();
        let (bytes, tile) = notification_avatar(Some(profile("Bob", Some(&photo))));
        assert_eq!(bytes, Some(photo));
        assert!(tile.is_none());
    }

    #[test]
    fn draws_a_text_avatar_in_its_own_colour() {
        let avatar = "data:application/x-dashchat-avatar,TextAvatarData|1|%23fbcfe8|GC";
        let tile = sender_letter_tile(Some(avatar), Some("Bob"));
        assert_eq!(tile.text.as_deref(), Some("GC"));
        assert_eq!(tile.background_color, "#fbcfe8");
        assert_eq!(tile.text_color, TEXT_AVATAR_TEXT_COLOR);
    }

    #[test]
    fn ignores_trailing_text_avatar_fields_like_the_app() {
        let avatar = "data:application/x-dashchat-avatar,TextAvatarData|1|%23fbcfe8|GC|x";
        assert_eq!(
            parse_text_avatar(avatar),
            Some(("#fbcfe8".to_string(), "GC".to_string()))
        );
    }

    #[test]
    fn falls_back_to_the_initials_of_a_malformed_text_avatar() {
        let avatar = "data:application/x-dashchat-avatar,TextAvatarData|1|%23fbcfe8|<b>";
        let tile = sender_letter_tile(Some(avatar), Some("Bob"));
        assert_eq!(tile.text.as_deref(), Some("BO"));
        assert_eq!(tile.background_color, GENERIC_AVATAR_GREY);
    }

    #[test]
    fn takes_the_first_two_characters_like_the_app() {
        assert_eq!(name_initials("bob"), "BO");
        assert_eq!(name_initials("🌸 Maria"), "🌸 ");
        assert_eq!(name_initials("مریم"), "مر");
    }

    #[test]
    fn draws_a_silhouette_while_waiting_for_the_profile() {
        let (bytes, tile) = notification_avatar(None);
        assert_eq!(bytes, None);
        assert_eq!(tile.unwrap().text, None);
    }

    fn profile(name: &str, avatar: Option<&str>) -> Profile {
        Profile {
            name: name.to_string(),
            surname: None,
            avatar: avatar.map(str::to_string),
            about: None,
        }
    }
}
