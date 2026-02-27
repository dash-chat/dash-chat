/**
 * Functions that navigate through pages, running checks at each stop.
 * Returns structured results for automated analysis.
 *
 * Split into three small functions to stay within MCP bridge's ~20s timeout:
 *   visitProfilePages  — home → settings → profile → sub-pages → home (~7 pages)
 *   visitOtherPages    — home → settings → appearance/account → home + new-message (~4 pages)
 *   visitChatPages     — home → direct-chat → chat-settings → home (~2 pages)
 *
 * visitAllPages combines all three (for E2E tests with longer timeouts).
 *
 * Uses click() from helpers.ts which handles the Konsta ListItem <a> pattern:
 *   clicks `selector + ' a'` first, falls back to `selector`.
 *
 * IMPORTANT: On desktop, many settings sub-pages hide their NavbarBackLink
 * (`{#if !isWideScreen.value}`). We use `testUtils.goto()` (SvelteKit's goto,
 * registered from +layout.svelte) for reliable back navigation, and content
 * selectors for page detection.
 */

import { S } from '../selectors';
import { waitFor, click } from '../helpers';
import { testUtils } from '../setup-utils';
import { checkPage } from './checks';
import type { CheckResult, PageResult } from './checks';

export interface VisitOptions {
	checkDarkMode?: boolean;
	checkRTL?: boolean;
	/** Visit direct chat pages (true after contact exchange + messaging). */
	hasChat?: boolean;
}

export interface VisitResult {
	pages: PageResult[];
	summary: { totalIssues: number; pagesVisited: number };
}

type CheckOpts = { checkDarkMode?: boolean; checkRTL?: boolean };

/** Shorter timeout for navigation waits (default 15s is too long for batched calls). */
const NAV_TIMEOUT = 8000;

function runCheck(pageName: string, options?: CheckOpts): PageResult {
	const result: CheckResult = checkPage(options);
	return { page: pageName, ...result };
}

function summarize(pages: PageResult[]): VisitResult {
	const totalIssues = pages.reduce((sum, p) => {
		let count = sum + p.overflow.length;
		if (p.darkMode) count += p.darkMode.issues.length;
		return count;
	}, 0);
	return { pages, summary: { totalIssues, pagesVisited: pages.length } };
}

/** Selector that matches the home page regardless of whether chats exist. */
const HOME = `${S.home.chatList}, ${S.home.emptyState}`;

/** Navigate using SvelteKit goto (clean, no history stack issues). */
async function nav(path: string, waitSelector: string): Promise<void> {
	await testUtils.goto(path);
	await waitFor(waitSelector, NAV_TIMEOUT);
}

/**
 * Visit profile-related pages: home → settings → profile → edit-name →
 * edit-about → edit-photo → add-contact → back to home.
 * ~7 page checks.
 */
export async function visitProfilePages(options?: VisitOptions): Promise<VisitResult> {
	const pages: PageResult[] = [];
	const co: CheckOpts = {
		checkDarkMode: options?.checkDarkMode,
		checkRTL: options?.checkRTL,
	};

	// Home
	await waitFor(HOME, NAV_TIMEOUT);
	pages.push(runCheck('home', co));

	// Settings
	click(S.home.settingsLink);
	await waitFor(S.settings.profileLink, NAV_TIMEOUT);
	pages.push(runCheck('settings', co));

	// Profile — wait for content element (profile-back hidden on desktop)
	click(S.settings.profileLink);
	await waitFor(S.profile.editName, NAV_TIMEOUT);
	pages.push(runCheck('profile', co));

	// Edit Name → back (edit-name-back is always visible)
	click(S.profile.editName);
	await waitFor(S.editName.back, NAV_TIMEOUT);
	pages.push(runCheck('edit-name', co));
	click(S.editName.back);
	await waitFor(S.profile.editName, NAV_TIMEOUT);

	// Edit About → back (edit-about-back is always visible)
	click(S.profile.editAbout);
	await waitFor(S.editAbout.back, NAV_TIMEOUT);
	pages.push(runCheck('edit-about', co));
	click(S.editAbout.back);
	await waitFor(S.profile.editName, NAV_TIMEOUT);

	// Edit Photo → close (edit-photo-close is always visible)
	click(S.profile.editPhoto);
	await waitFor(S.editPhoto.close, NAV_TIMEOUT);
	pages.push(runCheck('edit-photo', co));
	click(S.editPhoto.close);
	await waitFor(S.profile.editName, NAV_TIMEOUT);

	// Add Contact (from profile) — use copyButton (always present)
	click(S.profile.qrLink);
	await waitFor(S.addContact.copyButton, NAV_TIMEOUT);
	pages.push(runCheck('profile-add-contact', co));

	// Back to home via goto (clean, avoids hidden back buttons)
	await nav('/', HOME);

	return summarize(pages);
}

/**
 * Visit other settings + new-message pages: home → settings → appearance →
 * account → home → new-message → add-contact → home.
 * ~4 page checks.
 */
export async function visitOtherPages(options?: VisitOptions): Promise<VisitResult> {
	const pages: PageResult[] = [];
	const co: CheckOpts = {
		checkDarkMode: options?.checkDarkMode,
		checkRTL: options?.checkRTL,
	};

	await waitFor(HOME, NAV_TIMEOUT);

	// Appearance — use content selector (appearance-back hidden on desktop)
	await nav('/settings/appearance', S.appearance.light);
	pages.push(runCheck('appearance', co));

	// Account — navigate directly (account-back hidden on desktop)
	await nav('/settings/account', S.account.deleteItem);
	pages.push(runCheck('account', co));

	// Back to home
	await nav('/', HOME);

	// New Message (theme-agnostic: try FAB first, fall back to link)
	const fab = document.querySelector(S.home.newMessageFab) as HTMLElement | null;
	if (fab && fab.offsetWidth > 0) {
		fab.click();
	} else {
		click(S.home.newMessageLink);
	}
	await waitFor(S.newMessage.addContact, NAV_TIMEOUT);
	pages.push(runCheck('new-message', co));

	// Add Contact (from new-message) — use copyButton (always present)
	click(S.newMessage.addContact);
	await waitFor(S.addContact.copyButton, NAV_TIMEOUT);
	pages.push(runCheck('new-message-add-contact', co));

	// Back to home
	await nav('/', HOME);

	return summarize(pages);
}

/**
 * Visit direct chat pages: home → direct-chat → chat-settings → home.
 * Only runs if hasChat is true and a chat exists.
 * ~2 page checks.
 */
export async function visitChatPages(options?: VisitOptions): Promise<VisitResult> {
	const pages: PageResult[] = [];
	const co: CheckOpts = {
		checkDarkMode: options?.checkDarkMode,
		checkRTL: options?.checkRTL,
	};

	await waitFor(HOME, NAV_TIMEOUT);

	if (options?.hasChat) {
		const chatLink = document.querySelector(`${S.home.chatList} a`) as HTMLElement | null;
		if (chatLink) {
			chatLink.click();
			await waitFor(S.directChat.messages, NAV_TIMEOUT);
			pages.push(runCheck('direct-chat', co));

			// Chat settings → back (chat-settings-back is always visible)
			click(S.directChat.settingsLink);
			await waitFor(S.chatSettings.back, NAV_TIMEOUT);
			pages.push(runCheck('chat-settings', co));

			// Back to home via goto
			await nav('/', HOME);
		}
	}

	return summarize(pages);
}

/**
 * Convenience aliases for backward compatibility.
 */
export async function visitSettingsPages(options?: VisitOptions): Promise<VisitResult> {
	const profile = await visitProfilePages(options);
	const other = await visitOtherPages(options);
	const pages = [...profile.pages, ...other.pages];
	return summarize(pages);
}

/**
 * Visit ALL pages (profile + other + chat). Combines all functions.
 * Takes ~15-25s — use for E2E tests (longer timeout), NOT for MCP webview_execute_js.
 */
export async function visitAllPages(options?: VisitOptions): Promise<VisitResult> {
	const profile = await visitProfilePages(options);
	const other = await visitOtherPages(options);
	const chat = await visitChatPages(options);
	const pages = [...profile.pages, ...other.pages, ...chat.pages];
	return summarize(pages);
}
