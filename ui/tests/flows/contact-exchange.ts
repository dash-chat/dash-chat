import { S } from '../selectors';
import { waitFor, typeInto, click } from '../helpers';

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
	newMessageFab: S.home.newMessageFab,
	newMessageLink: S.home.newMessageLink,
	addContactItem: S.newMessage.addContact,
	copyButton: S.addContact.copyButton,
	codeInput: `${S.addContact.codeInput} input`,
};

/** Navigate from home to the add-contact page via the UI. */
export async function navigateToAddContact(): Promise<true> {
	// On narrow screens (Material): FAB is visible
	// On narrow screens (iOS) or wide screens: navbar link is visible
	const fab = document.querySelector(steps.newMessageFab);
	if (fab) {
		(fab as HTMLElement).click();
	} else {
		const link = document.querySelector(steps.newMessageLink) as HTMLElement | null;
		if (!link) {
			throw new Error(
				`navigateToAddContact: neither FAB (${steps.newMessageFab}) nor link (${steps.newMessageLink}) found in DOM`,
			);
		}
		link.click();
	}
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
