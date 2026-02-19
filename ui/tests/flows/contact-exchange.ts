import { S } from '../selectors';
import { waitFor, typeInto, click } from '../helpers';

/**
 * Contact exchange flow between two agents.
 *
 * Precondition: Both agents have created profiles.
 *
 * Steps:
 *   1. On Agent 1: Navigate to add-contact page
 *      - From home: click S.home.contactsLink -> then S.contacts.addLink
 *      - Or directly navigate to /add-contact
 *
 *   2. On Agent 1: Copy the contact code
 *      - Wait for: S.addContact.copyButton
 *      - Get the QR code value: document.querySelector('wa-qr-code')?.getAttribute('value')
 *
 *   3. On Agent 2: Navigate to add-contact page (same as step 1)
 *
 *   4. On Agent 2: Paste Agent 1's code
 *      - Type into: S.addContact.codeInput + ' input'
 *      - This triggers automatic navigation to the direct chat
 *
 *   5. On Agent 1: Paste Agent 2's code (same as step 4 with swapped codes)
 *
 *   6. Verify: Both agents should see a direct chat with the other
 */

export const steps = {
	contactsLink: S.home.contactsLink,
	addContactLink: S.contacts.addLink,
	copyButton: S.addContact.copyButton,
	codeInput: `${S.addContact.codeInput} input`,
	getCodeScript: `document.querySelector('wa-qr-code')?.value`,
};

/** Navigate from home to the add-contact page. */
export async function navigateToAddContact(): Promise<true> {
	click(steps.contactsLink);
	await waitFor(steps.addContactLink);
	click(steps.addContactLink);
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
	await waitFor(S.directChat.messages);
	return true;
}
