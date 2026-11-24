import { m } from '$lib/paraglide/messages.js';
import { showToast } from '$lib/utils/toasts';
import { type Message, type MessagesStore } from 'dash-chat-stores';

/** Toggles my emoji reaction on the message, showing a toast on failure. */
export async function toggleReaction(
	store: MessagesStore,
	message: Message,
	emoji: string,
) {
	try {
		await store.toggleReaction(message, emoji);
	} catch (e) {
		console.error('Failed to toggle reaction', e);
		showToast(m.errorUnexpected(), 'unexpected', e);
	}
}
