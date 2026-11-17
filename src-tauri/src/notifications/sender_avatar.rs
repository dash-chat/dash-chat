use base64::Engine;
use image::Rgb;

const SIZE: u32 = 192;
/// Keep in sync with `text-avatar-data-url.ts`.
const TEXT_AVATAR_PREFIX: &str = "data:application/x-dashchat-avatar,TextAvatarData|1|";
/// Keep in sync with `TEXT_AVATAR_TEXT_COLOR` in `Avatar.svelte`.
const TEXT_AVATAR_TEXT_COLOR: Rgb<u8> = Rgb([0x83, 0x18, 0x43]);
const GENERIC_AVATAR_GREY: Rgb<u8> = Rgb([0x8e, 0x8e, 0x93]);
const WHITE: Rgb<u8> = Rgb([0xff, 0xff, 0xff]);
const PERSON_AVATAR: &str = concat!(
    "data:image/png;base64,",
    include_str!("person-avatar.png.base64")
);

/// The image a notification shows for its sender: their photo, or on Android
/// what the app draws for them.
///
/// Without an image Android draws the conversation with a blank black avatar
/// and no app badge, while iOS draws its own initials.
pub(super) fn sender_avatar_image(avatar: Option<&str>, name: Option<&str>) -> Option<String> {
    if let Some(photo) = avatar.filter(|avatar| avatar.starts_with("data:image/")) {
        return Some(photo.to_string());
    }
    cfg!(target_os = "android").then(|| drawn_avatar_image(avatar, name))
}

/// Like Signal: the text avatar, else the initials of the name, else a person
/// silhouette.
fn drawn_avatar_image(avatar: Option<&str>, name: Option<&str>) -> String {
    let png = match avatar.and_then(parse_text_avatar) {
        Some((background, text)) => {
            text_avatar::text_avatar_png(&text, SIZE, background, TEXT_AVATAR_TEXT_COLOR)
        }
        None => name.and_then(|name| {
            text_avatar::text_avatar_png(&name_initials(name), SIZE, GENERIC_AVATAR_GREY, WHITE)
        }),
    };
    match png {
        Some(png) => format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ),
        None => PERSON_AVATAR.to_string(),
    }
}

fn parse_text_avatar(avatar: &str) -> Option<(Rgb<u8>, String)> {
    let (encoded_color, text) = avatar.strip_prefix(TEXT_AVATAR_PREFIX)?.split_once('|')?;
    let hex = encoded_color.strip_prefix("%23")?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let color = Rgb([channel(0)?, channel(2)?, channel(4)?]);
    let text_is_valid = (1..=3).contains(&text.len())
        && text
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    text_is_valid.then(|| (color, text.to_string()))
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
    fn keeps_a_photo() {
        let photo = "data:image/jpeg;base64,AAAA";
        assert_eq!(
            sender_avatar_image(Some(photo), Some("Bob")).as_deref(),
            Some(photo)
        );
    }

    #[test]
    fn reads_a_text_avatar() {
        let avatar = "data:application/x-dashchat-avatar,TextAvatarData|1|%23fbcfe8|GC";
        assert_eq!(
            parse_text_avatar(avatar),
            Some((Rgb([0xfb, 0xcf, 0xe8]), "GC".to_string()))
        );
    }

    #[test]
    fn rejects_a_malformed_text_avatar() {
        let avatar = "data:application/x-dashchat-avatar,TextAvatarData|1|%23fbcfe8|<b>";
        assert_eq!(parse_text_avatar(avatar), None);
    }

    #[test]
    fn takes_the_letters_among_the_first_two_characters() {
        assert_eq!(name_initials(" bob "), "BO");
        assert_eq!(name_initials("J Doe"), "J");
        assert_eq!(name_initials("J.R."), "J");
        assert_eq!(name_initials("άννα"), "ΆΝ");
    }

    #[test]
    fn falls_back_to_the_person_silhouette() {
        assert_eq!(drawn_avatar_image(None, None), PERSON_AVATAR);
        assert_eq!(drawn_avatar_image(None, Some("مریم")), PERSON_AVATAR);
        assert_eq!(drawn_avatar_image(None, Some("..")), PERSON_AVATAR);
        assert_ne!(drawn_avatar_image(None, Some("Bob")), PERSON_AVATAR);
    }
}
