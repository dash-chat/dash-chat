use dashchat_node::Profile;
use tauri_plugin_notification::LetterTile;

/// Keep in sync with `text-avatar-data-url.ts`.
const TEXT_AVATAR_PREFIX: &str = "data:application/x-dashchat-avatar,TextAvatarData|1|";
/// Keep in sync with `TEXT_AVATAR_TEXT_COLOR` in `Avatar.svelte`.
const TEXT_AVATAR_TEXT_COLOR: &str = "#831843";
/// The light theme's neutral avatar colours (`--wa-color-neutral-fill-normal`
/// and `--wa-color-neutral-on-normal`), which the app draws initials in.
const NEUTRAL_AVATAR_BACKGROUND: &str = "#e4e5e9";
const NEUTRAL_AVATAR_FOREGROUND: &str = "#424554";
/// The app's avatar while it waits for a profile: the account outline icon in
/// the neutral avatar colours.
const WAITING_FOR_PROFILE_AVATAR: &str = concat!(
    "data:image/png;base64,",
    include_str!("waiting-for-profile-avatar.png.base64")
);

/// The sender's photo as `large_icon_bytes`, or else, like Signal, what the
/// app draws for them: their text avatar or initials as `large_icon_letter_tile`,
/// or its waiting-for-profile icon when it has no profile or name to draw.
pub(super) fn notification_avatar(
    profile: Option<Profile>,
) -> (Option<String>, Option<LetterTile>) {
    let tile = match profile {
        Some(Profile {
            avatar: Some(photo),
            ..
        }) if photo.starts_with("data:image/") => return (Some(photo), None),
        Some(profile) => sender_letter_tile(profile.avatar.as_deref(), &profile.name),
        None => None,
    };
    match tile {
        Some(tile) => (None, Some(tile)),
        None => (Some(WAITING_FOR_PROFILE_AVATAR.to_string()), None),
    }
}

/// Their text avatar, else the initials of their name.
fn sender_letter_tile(avatar: Option<&str>, name: &str) -> Option<LetterTile> {
    if let Some((background_color, text)) = avatar.and_then(parse_text_avatar) {
        return Some(LetterTile {
            text,
            background_color,
            text_color: TEXT_AVATAR_TEXT_COLOR.to_string(),
        });
    }
    let initials = name_initials(name);
    (!initials.trim().is_empty()).then(|| LetterTile {
        text: initials,
        background_color: NEUTRAL_AVATAR_BACKGROUND.to_string(),
        text_color: NEUTRAL_AVATAR_FOREGROUND.to_string(),
    })
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

    fn profile(name: &str, avatar: Option<&str>) -> Profile {
        Profile {
            name: name.to_string(),
            surname: None,
            avatar: avatar.map(str::to_string),
            about: None,
        }
    }

    #[test]
    fn keeps_a_photo() {
        let photo = "data:image/jpeg;base64,AAAA";
        let (bytes, tile) = notification_avatar(Some(profile("Bob", Some(photo))));
        assert_eq!(bytes.as_deref(), Some(photo));
        assert!(tile.is_none());
    }

    #[test]
    fn draws_a_text_avatar_in_its_own_colour() {
        let avatar = "data:application/x-dashchat-avatar,TextAvatarData|1|%23fbcfe8|GC";
        let tile = sender_letter_tile(Some(avatar), "Bob").unwrap();
        assert_eq!(tile.text, "GC");
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
        let tile = sender_letter_tile(Some(avatar), "Bob").unwrap();
        assert_eq!(tile.text, "BO");
        assert_eq!(tile.background_color, NEUTRAL_AVATAR_BACKGROUND);
    }

    #[test]
    fn takes_the_first_two_characters_like_the_app() {
        assert_eq!(name_initials("bob"), "BO");
        assert_eq!(name_initials("🌸 Maria"), "🌸 ");
        assert_eq!(name_initials("مریم"), "مر");
    }

    #[test]
    fn shows_the_waiting_for_profile_icon_without_a_profile_or_name() {
        for sender in [None, Some(profile(" ", None))] {
            let (bytes, tile) = notification_avatar(sender);
            assert_eq!(bytes.as_deref(), Some(WAITING_FOR_PROFILE_AVATAR));
            assert!(tile.is_none());
        }
    }
}
