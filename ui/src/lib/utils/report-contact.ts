import { m } from '$lib/paraglide/messages.js';
import { showToast } from '$lib/utils/toasts';
import { type AgentId, type ContactsStore } from 'dash-chat-stores';

/** Report a contact and surface the outcome as a toast. */
export async function reportContactWithFeedback(
	contactsStore: ContactsStore,
	agentId: AgentId,
) {
	try {
		const mailboxes = await contactsStore.reportContact(agentId);
		showToast(mailboxes.length > 0 ? m.reported() : m.reportNoMailboxReached());
	} catch (e) {
		console.error(e);
		showToast(m.errorUnexpected(), 'unexpected', e);
	}
}
