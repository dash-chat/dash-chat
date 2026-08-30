import { goto } from '$app/navigation';
import { m } from '$lib/paraglide/messages.js';
import { addContactPending } from '$lib/stores/add-contact-pending.svelte';
import { showToast } from '$lib/utils/toasts';
import { type AddContactError, type ContactsStore } from 'dash-chat-stores';

import { buildHttpsDeepLinkUrl, extractDeepLinkParams } from './helpers';

export const path = '/add-contact/{{code}}';

export function toDeepLink(code: string): string | null {
	return buildHttpsDeepLinkUrl(path, { code });
}

export function extractCodeFromDeepLink(input: string): string | null {
	let params = extractDeepLinkParams(input, path);
	return params?.code ?? null;
}

export async function addContactFromDeepLink(
	contactsStore: ContactsStore,
	link: string,
): Promise<void> {
	const code = extractCodeFromDeepLink(link.trim());
	if (code === null) {
		showToast(m.errorAddContactInvalidLink(), 'error');
		return;
	}

	await addContactFromCode(contactsStore, code);
}

// A duplicate delivery of the same code (a re-delivered deep link, a double
// tap) arrives while the first one is still being handled — drop it. A
// genuine repeat click lands after handling finished and passes through.
let inFlightCode: string | null = null;

export async function addContactFromCode(
	contactsStore: ContactsStore,
	code: string,
): Promise<void> {
	if (code === inFlightCode) return;
	inFlightCode = code;
	addContactPending.value = true;
	try {
		const result = await contactsStore.client.addContact(code);
		await goto(`/direct-chats/${result.chatId}`);

		if (result.kind === 'NewRequest') {
			showToast(m.contactRequestSent());
		}
	} catch (e) {
		console.error(e);
		const error = e as AddContactError;
		switch (error.kind) {
			case 'ProfileNotCreated':
				showToast(m.errorAddContactProfileRequired(), 'error');
				break;
			case 'CannotAddSelf':
				showToast(m.cantAddYourselfAsContact(), 'error');
				break;
			case 'InvalidContactCode':
				showToast(m.errorAddContactInvalidLink(), 'error');
				break;
			case 'InitializeTopic':
			case 'AuthorOperation':
			case 'CreateQrCode':
			case 'CreateDirectChat':
			case 'StoreContact':
				showToast(m.errorAddContact(), 'error');
				break;
			default:
				showToast(m.errorUnexpected(), 'unexpected', e);
		}
	} finally {
		inFlightCode = null;
		addContactPending.value = false;
	}
}

export function handle(
	{ code }: Record<string, string>,
	contactsStore: ContactsStore,
) {
	void addContactFromCode(contactsStore, code);
}
