import { goto } from '$app/navigation';
import { m } from '$lib/paraglide/messages.js';
import { showToast } from '$lib/utils/toasts';
import {
	type AddContactError,
	type ContactsStore,
	pendingChatKey,
} from 'dash-chat-stores';

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

export async function addContactFromCode(
	contactsStore: ContactsStore,
	code: string,
): Promise<void> {
	try {
		const devicePubkey = await contactsStore.client.addContact(code);
		showToast(m.contactRequestSent());

		const knownAgent = await contactsStore.client.agentForDevice(devicePubkey);
		goto(`/direct-chats/${knownAgent ?? pendingChatKey(devicePubkey)}`);
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
			case 'AddDeviceNotSupported':
				showToast(m.errorAddContactDeviceLinkingNotSupported(), 'error');
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
	}
}

export function handle(
	{ code }: Record<string, string>,
	contactsStore: ContactsStore,
) {
	void addContactFromCode(contactsStore, code);
}
