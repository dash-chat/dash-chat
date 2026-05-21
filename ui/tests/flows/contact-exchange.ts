import { click, typeInto, waitFor } from '../helpers';
import { S } from '../selectors';

/**
 * Contact exchange flow between two agents.
 *
 * Precondition: Both agents have created profiles and are on the home page.
 *
 * Steps:
 *   1. Click new-message FAB to go to /new-message
 *   2. Click add-contact item to go to /new-message/add-contact
 *   3. Read the contact code from the QR element
 *   4. Paste the other agent's code into the input
 *   5. Wait for the direct chat to open
 */

export const steps = {
	newMessageButton: S.home.newMessageButton,
	addContactItem: S.newMessage.addContact,
	copyButton: S.addContact.copyButton,
	codeInput: `${S.addContact.codeInput} input`,
};

/** Navigate from home to the add-contact page via the UI. */
export async function navigateToAddContact(): Promise<true> {
	await waitFor(steps.newMessageButton);
	click(steps.newMessageButton);
	await waitFor(steps.addContactItem);
	click(steps.addContactItem);
	await waitFor(steps.codeInput);
	return true;
}

/** Read the contact code from the QR element. */
export function getContactCode(): string | null {
	return (document.querySelector(S.addContact.qrCode) as any)?.value ?? null;
}

/** Paste a contact code and wait for the direct chat to open. */
export async function addContact(contactCode: string): Promise<true> {
	typeInto(steps.codeInput, contactCode);
	await waitFor(S.directChat.page);
	return true;
}
