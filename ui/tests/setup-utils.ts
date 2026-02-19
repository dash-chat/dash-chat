/**
 * Registers test utilities on `window.__test` for UI automation
 * via webview_execute_js in dev mode.
 *
 * Usage:
 *   await window.__test.createProfile('Alice', 'Test')
 *   await window.__test.navigateToAddContact()
 *   window.__test.getContactCode()
 *   await window.__test.addContact('<code>')
 *   window.__test.sendMessage('Hello!')
 *   await window.__test.waitForMessage('Hello!')
 */

import { waitFor, waitForText, typeInto, click, nextTick } from './helpers';
import { createProfile } from './flows/profile-creation';
import { navigateToAddContact, getContactCode, addContact } from './flows/contact-exchange';
import { sendMessage, waitForMessage } from './flows/send-message';

export const testUtils = {
	waitFor,
	waitForText,
	typeInto,
	click,
	nextTick,
	createProfile,
	navigateToAddContact,
	getContactCode,
	addContact,
	sendMessage,
	waitForMessage,
};

declare global {
	interface Window {
		__test: typeof testUtils;
	}
}

export function registerTestUtils() {
	window.__test = testUtils;
}
