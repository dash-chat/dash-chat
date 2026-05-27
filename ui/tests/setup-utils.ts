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
import type { m } from '../src/lib/paraglide/messages.js';
import { previewFeatures } from '$lib/stores/preview-features.svelte';
import {
	addContact,
	getContactCode,
	navigateToAddContact,
} from './flows/contact-exchange';
import { setLocalMailboxEnabled } from './flows/local-mailbox';
import { openDirectChat } from './flows/open-chat';
import { createProfile } from './flows/profile-creation';
import { sendMessage, waitForMessage } from './flows/send-message';
import {
	captureNextToastMessage,
	click,
	nextTick,
	typeInto,
	waitFor,
	waitForText,
} from './helpers';
import { uploadEmptyImage, uploadQrCodeImage } from './pages/add-contact';
import { chatSettingsLoaded } from './pages/chat-settings';
import {
	chatOverflow,
	checkNavbarOverflow,
	clickScrollBottomButton,
	connectionStatus,
	isContactRequestBannerVisible,
	isPeerNamePresent,
	isScrollAtBottom,
	lastMessageStatus,
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
	clickNewMessage,
	firstChatTooltip,
	getChatListItem,
	hasChatListItem,
	homeLoaded,
} from './pages/home';
import {
	clickNewGroupCreate,
	clickNewGroupNext,
	newGroupLoaded,
} from './pages/new-group';
import { clickNewGroup, newMessageLoaded } from './pages/new-message';
import { isPeerProfileSheetOpen } from './pages/peer-profile-sheet';
import { profileNameListItemContains } from './pages/profile-settings';
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

type Messages = typeof m;
type MessageKey = Extract<keyof Messages, string>;
type MessageParams<K extends MessageKey> = Parameters<Messages[K]>[0];

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
	uploadQrCodeImage,
	uploadEmptyImage,
	captureNextToastMessage,
	sendMessage,
	waitForMessage,
	openDirectChat,
	getStartedCards,
	dismissGetStartedCard,
	clickNewMessage,
	homeLoaded,
	firstChatTooltip,
	getChatListItem,
	hasChatListItem,
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
	isContactRequestBannerVisible,
	checkNavbarOverflow,
	lastMessageStatus,
	connectionStatus,
	setLocalMailboxEnabled,
	chatSettingsLoaded,
	isPeerProfileSheetOpen,
	profileNameListItemContains,
	versionItem,
	updaterBanner,
	updaterBannerTitle,
	updaterDismissBtn,
	simulateUpdate,
	clickNewGroup,
	newMessageLoaded,
	newGroupLoaded,
	clickNewGroupNext,
	clickNewGroupCreate,
	/** Resolve a paraglide message in the current locale (set by registerTestUtils). */
	tr<K extends MessageKey>(key: K, _params?: MessageParams<K>): string {
		throw new Error(
			`tr(${JSON.stringify(key)}) called before registerTestUtils provided messages`,
		);
	},
	checkOverflow,
	checkDarkMode,
	checkRTL,
	checkPage,
	visitAllPages,
	visitSettingsPages,
	visitProfilePages,
	visitOtherPages,
	visitChatPages,
	/** Paraglide setLocale — set by registerTestUtils from +layout.svelte. */
	setLocale: (_locale: string) => {},
	/** SvelteKit goto — set by registerTestUtils from +layout.svelte. */
	goto: (_path: string) => Promise.resolve() as Promise<void>,
};

declare global {
	interface Window {
		__test: typeof testUtils;
	}
}

export function registerTestUtils(
	goto?: (path: string) => Promise<void>,
	setLocale?: (locale: string) => void,
	messages?: Messages,
) {
	previewFeatures.enable();
	window.__test = testUtils;
	if (goto) {
		testUtils.goto = goto;
	}
	if (setLocale) {
		testUtils.setLocale = setLocale;
	}
	if (messages) {
		testUtils.tr = <K extends MessageKey>(
			key: K,
			params?: MessageParams<K>,
		): string => {
			const message = messages[key] as
				| ((inputs: MessageParams<K>) => string)
				| undefined;
			if (!message) {
				throw new Error(`tr: missing paraglide message for key "${key}"`);
			}
			const value = message((params ?? {}) as MessageParams<K>);
			if (!value) {
				throw new Error(`tr: paraglide message for key "${key}" is empty`);
			}
			return value;
		};
	}
}
