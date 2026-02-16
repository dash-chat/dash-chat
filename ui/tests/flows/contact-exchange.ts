import { S } from '../selectors';

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
	getCodeScript: `document.querySelector('wa-qr-code')?.getAttribute('value')`,
};
