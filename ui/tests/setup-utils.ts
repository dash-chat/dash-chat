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
import {
	addContact,
	getContactCode,
	navigateToAddContact,
} from './flows/contact-exchange';
import { openDirectChat } from './flows/open-chat';
import { createProfile } from './flows/profile-creation';
import { sendMessage, waitForMessage } from './flows/send-message';
import { click, nextTick, typeInto, waitFor, waitForText } from './helpers';
import {
	chatOverflow,
	checkNavbarOverflow,
	clickScrollBottomButton,
	isPeerNamePresent,
	isScrollAtBottom,
	messageInput,
	messagesContainer,
	navbarBgOpacity,
	scrollBottomButtonVisible,
	scrollChatToBottom,
	scrollChatToTop,
	scrollChatUp,
	sendButton,
	unreadBadgeText,
} from './pages/direct-chat';
import {
	dismissCard as dismissGetStartedCard,
	visibleCards as getStartedCards,
} from './pages/get-started';
import { versionItem } from './pages/help';
import {
	checkChatListOverflow,
	firstChatTooltip,
	homeLoaded,
} from './pages/home';
import {
	updaterBanner,
	updaterBannerTitle,
	updaterDismissBtn,
} from './pages/updater-banner';
import {
	checkDarkMode,
	checkOverflow,
	checkPage,
	checkRTL,
} from './review/checks';
import {
	visitAllPages,
	visitChatPages,
	visitOtherPages,
	visitProfilePages,
	visitSettingsPages,
} from './review/visit-all-pages';

/** Trigger UpdaterBanner into a specific state via custom event. */
function simulateUpdate(
	state: 'available' | 'downloading' | 'ready' | 'error' | 'hidden',
) {
	window.dispatchEvent(
		new CustomEvent('test-simulate-update', { detail: state }),
	);
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
	isScrollAtBottom,
	chatOverflow,
	scrollChatUp,
	scrollBottomButtonVisible,
	unreadBadgeText,
	clickScrollBottomButton,
	scrollChatToBottom,
	scrollChatToTop,
	navbarBgOpacity,
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
