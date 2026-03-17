const AVATAR_DATA_URL_PREFIX = 'data:application/x-dashchat-avatar,';
const TEXT_AVATAR_TYPE_NAME = 'TextAvatarData';
const TEXT_AVATAR_VERSION = '1';
const COLOR_REGEX = /^#[0-9a-f]{6}$/i;
const TEXT_REGEX = /^[A-Z0-9]{1,3}$/;

export class TextAvatarData {
	constructor(
		public color: string,
		public text: string,
	) {}

  sanitizedHexColor(): string {
    // Ensure the color is a valid hex code and sanitize it
    const hexColorRegex = /^#([0-9A-Fa-f]{3}){1,2}$/;
    if (hexColorRegex.test(this.color)) {
      return this.color;
    }
    // Fallback to a default color if invalid
    return '#cccccc';
  }

	serialize(): string {
		return `${AVATAR_DATA_URL_PREFIX}${TEXT_AVATAR_TYPE_NAME}|${TEXT_AVATAR_VERSION}|${encodeURIComponent(this.color)}|${encodeURIComponent(this.text)}`;
	}

	static deserialize(value: string | undefined): TextAvatarData | undefined {
		if (!value) {
			return undefined;
		}

		if (value.startsWith(AVATAR_DATA_URL_PREFIX)) {
			const payload = value.slice(AVATAR_DATA_URL_PREFIX.length);
			const [typeName = '', version = '', encodedColor = '', encodedText = ''] =
				payload.split('|', 4);

			if (
				typeName !== TEXT_AVATAR_TYPE_NAME ||
				version !== TEXT_AVATAR_VERSION ||
				!encodedColor ||
				!encodedText
			) {
				return undefined;
			}

			try {
				const color = decodeURIComponent(encodedColor);
				const text = decodeURIComponent(encodedText);

				if (!color || !text) {
					return undefined;
				}

				if (!COLOR_REGEX.test(color) || !TEXT_REGEX.test(text)) {
					return undefined;
				}

				return new TextAvatarData(color, text);
			} catch {
				return undefined;
			}
		}

		return undefined;
	}
}