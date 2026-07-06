import { m } from '$lib/paraglide/messages.js';
import { showToast } from '$lib/utils/toasts';
import type { DeviceId, Message, MessagesStore } from 'dash-chat-stores';

/** Adds the emoji reaction to the message, or removes it when the device has
 * already reacted with that same emoji. */
export async function toggleReaction(
	store: MessagesStore,
	message: Message,
	myDeviceId: DeviceId,
	emoji: string,
) {
	const newEmoji = message.reactions[myDeviceId] === emoji ? null : emoji;
	try {
		await store.sendReaction({ target: message.hash, emoji: newEmoji });
	} catch (e) {
		console.error('Failed to toggle reaction', e);
		showToast(m.errorUnexpected(), 'unexpected', e);
	}
}
