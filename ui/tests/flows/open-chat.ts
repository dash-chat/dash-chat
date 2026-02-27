import { S } from '../selectors';
import { waitFor } from '../helpers';

/**
 * Open a direct chat from the home chat list by contact name.
 *
 * Precondition: Agent is on the home page with at least one chat visible.
 *
 * Finds the contact's link in the chat list and clicks it,
 * then waits for the messages container to appear.
 */
export async function openDirectChat(contactName: string): Promise<void> {
	const list = await waitFor(S.home.chatList);
	const links = Array.from(list.querySelectorAll('a'));
	for (const link of links) {
		if (link.textContent?.includes(contactName)) {
			(link as HTMLElement).click();
			await waitFor(S.directChat.messages);
			return;
		}
	}
	throw new Error(`openDirectChat: no chat with "${contactName}" in chat list`);
}
