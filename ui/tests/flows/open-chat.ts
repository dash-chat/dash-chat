import { waitFor } from '../helpers';
import { getChatListItem } from '../pages/home';
import { S } from '../selectors';

/**
 * Open a direct chat from the home chat list by contact name.
 *
 * Precondition: Agent is on the home page with at least one chat visible.
 *
 * Finds the contact's link in the chat list and clicks it,
 * then waits for the messages container to appear.
 */
export async function openDirectChat(contactName: string): Promise<void> {
	await waitFor(S.home.chatList);
	const item = getChatListItem(contactName);
	if (!item) {
		throw new Error(`openDirectChat: no chat with "${contactName}" in chat list`);
	}
	(item as HTMLElement).click();
	await waitFor(S.directChat.messages);
}
