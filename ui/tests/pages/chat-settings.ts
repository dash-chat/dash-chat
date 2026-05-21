import { S } from '../selectors';

export const selectors = S.chatSettings;

/** True if the chat-settings page has rendered (peer-name element is present). */
export function chatSettingsLoaded(): boolean {
	return !!document.querySelector(selectors.peerName);
}
