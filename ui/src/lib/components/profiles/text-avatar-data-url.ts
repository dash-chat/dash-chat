const AVATAR_DATA_URL_PREFIX = 'data:application/x-dashchat-avatar,';
const TEXT_AVATAR_TYPE_NAME = 'TextAvatarData';
const TEXT_AVATAR_VERSION = '1';
const COLOR_REGEX = /^#[0-9a-fA-F]{6}$/i;
const TEXT_REGEX = /^[A-Z0-9]{1,3}$/;

export class TextAvatarData {
	constructor(
		public readonly color: string,
		public readonly text: string,
	) {}

	private static validate(color: string, text: string): boolean {
		return !!color && !!text && COLOR_REGEX.test(color) && TEXT_REGEX.test(text);
	}

	sanitizedHexColor(): string {
		// Ensure the color is a valid hex code and sanitize it
		if (COLOR_REGEX.test(this.color)) {
			return this.color;
		}
		// Fallback to a default color if invalid
		return '#cccccc';
	}

	serialize(): string | undefined {
		if (!TextAvatarData.validate(this.color, this.text)) {
			return undefined;
		}

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
				version !== TEXT_AVATAR_VERSION
			) {
				return undefined;
			}

			try {
				const color = decodeURIComponent(encodedColor);
				const text = decodeURIComponent(encodedText);

				if (!TextAvatarData.validate(color, text)) {
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
