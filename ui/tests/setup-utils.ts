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
import { openDirectChat } from './flows/open-chat';
import { visibleCards as getStartedCards, dismissCard as dismissGetStartedCard } from './pages/get-started';
import { homeLoaded, firstChatTooltip, checkChatListOverflow } from './pages/home';
import { messageInput, sendButton, messagesContainer, isPeerNamePresent, checkNavbarOverflow } from './pages/direct-chat';
import { versionItem } from './pages/help';
import { updaterBanner, updaterBannerTitle, updaterDismissBtn } from './pages/updater-banner';
import { checkOverflow, checkDarkMode, checkRTL, checkPage } from './review/checks';
import {
	visitAllPages,
	visitSettingsPages,
	visitProfilePages,
	visitOtherPages,
	visitChatPages,
} from './review/visit-all-pages';

/** Trigger UpdaterBanner into a specific state via custom event. */
function simulateUpdate(state: 'available' | 'downloading' | 'ready' | 'error' | 'hidden') {
	window.dispatchEvent(new CustomEvent('test-simulate-update', { detail: state }));
}

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
	openDirectChat,
	getStartedCards,
	dismissGetStartedCard,
	homeLoaded,
	firstChatTooltip,
	checkChatListOverflow,
	messageInput,
	sendButton,
	messagesContainer,
	isPeerNamePresent,
	checkNavbarOverflow,
	versionItem,
	updaterBanner,
	updaterBannerTitle,
	updaterDismissBtn,
	simulateUpdate,
	checkOverflow,
	checkDarkMode,
	checkRTL,
	checkPage,
	visitAllPages,
	visitSettingsPages,
	visitProfilePages,
	visitOtherPages,
	visitChatPages,
	/** SvelteKit goto — set by registerTestUtils from +layout.svelte. */
	goto: (_path: string) => Promise.resolve() as Promise<void>,
};

declare global {
	interface Window {
		__test: typeof testUtils;
	}
}

export function registerTestUtils(goto?: (path: string) => Promise<void>) {
	window.__test = testUtils;
	if (goto) {
		testUtils.goto = goto;
	}
}
