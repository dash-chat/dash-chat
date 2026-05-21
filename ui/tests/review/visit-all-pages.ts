/**
 * Functions that navigate through pages, running checks at each stop.
 * Returns structured results for automated analysis.
 *
 * Split into three small functions to stay within MCP bridge's ~20s timeout:
 *   visitProfilePages  — home → settings → profile → sub-pages → home (~7 pages)
 *   visitOtherPages    — home → settings → offline/appearance/help/contact-us/account → home + new-message (~7 pages)
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

/** Timeout for navigation waits. Generous to handle component remounts from
 *  isWideScreen changes, where reactive store subscriptions may take time to settle. */
const NAV_TIMEOUT = 30_000;

/** Yield to the event loop to prevent WebKitGTK from freezing.
 *  Heavy synchronous DOM operations (checkOverflow scans all elements with
 *  layout-triggering properties) can lock up WebKit if done back-to-back. */
function breathe(): Promise<void> {
	return new Promise(r => setTimeout(r, 200));
}

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

/** Update progress tracker (read by runVisit's hard timeout for diagnostics). */
function progress(step: string): void {
	(window as any).__visitProgress = step;
}

/** Navigate using SvelteKit goto with a timeout guard.
 *  SvelteKit's goto() can hang if the target page's load never completes.
 *  Includes a breathe() after navigation to let WebKit's rendering pipeline
 *  settle before heavy DOM operations (checkOverflow, checkDarkMode). */
async function nav(path: string, waitSelector: string): Promise<void> {
	progress(`goto:${path}`);
	await Promise.race([
		testUtils.goto(path),
		new Promise<never>((_, reject) =>
			setTimeout(() => reject(new Error(`goto("${path}") timed out after ${NAV_TIMEOUT}ms`)), NAV_TIMEOUT),
		),
	]);
	progress(`waitFor:${waitSelector}`);
	await waitFor(waitSelector, NAV_TIMEOUT);
	await breathe();
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
	progress('profile:home');
	await waitFor(HOME, NAV_TIMEOUT);
	// Settle pending paints/transitions before the first check — switchCombo's
	// dark-mode + theme changes can still be propagating when we land here.
	await breathe();
	pages.push(runCheck('home', co));
	await breathe();

	// Home with FirstChatTooltip visible — only possible when chat list is empty
	// (tooltip is gated behind chats.length === 0 and !isWideScreen). Skip when
	// chats exist since the tooltip won't render regardless of localStorage.
	if (document.querySelector(S.home.emptyState)) {
		progress('profile:home-tooltip');
		localStorage.removeItem('first-chat-tooltip-shown');
		await nav('/settings', S.settings.profileLink);
		await nav('/', HOME);
		await waitFor(S.home.firstChatTooltip, NAV_TIMEOUT);
		pages.push(runCheck('home-with-tooltip', co));
		// Dismiss tooltip (click restores the localStorage flag internally)
		(document.querySelector(S.home.firstChatTooltip) as HTMLElement)?.click();
		await breathe();
	}

	// Settings
	progress('profile:settings-click');
	click(S.home.settingsLink);
	await waitFor(S.settings.profileLink, NAV_TIMEOUT);
	pages.push(runCheck('settings', co));
	await breathe();

	// Profile — wait for content element (profile-back hidden on desktop)
	progress('profile:profile-click');
	click(S.settings.profileLink);
	await waitFor(S.profile.editName, NAV_TIMEOUT);
	pages.push(runCheck('profile', co));
	await breathe();

	// Edit Name → back (edit-name-back is always visible)
	click(S.profile.editName);
	await waitFor(S.editName.back, NAV_TIMEOUT);
	pages.push(runCheck('edit-name', co));
	await breathe();
	click(S.editName.back);
	await waitFor(S.profile.editName, NAV_TIMEOUT);

	// Edit About → back (edit-about-back is always visible)
	click(S.profile.editAbout);
	await waitFor(S.editAbout.back, NAV_TIMEOUT);
	pages.push(runCheck('edit-about', co));
	await breathe();
	click(S.editAbout.back);
	await waitFor(S.profile.editName, NAV_TIMEOUT);

	// Edit Photo → close (edit-photo-close is always visible)
	click(S.profile.editPhoto);
	await waitFor(S.editPhoto.close, NAV_TIMEOUT);
	pages.push(runCheck('edit-photo', co));
	await breathe();
	click(S.editPhoto.close);
	await waitFor(S.profile.editName, NAV_TIMEOUT);

	// Add Contact (from profile) — use copyButton (always present)
	click(S.profile.qrLink);
	await waitFor(S.addContact.copyButton, NAV_TIMEOUT);
	pages.push(runCheck('profile-add-contact', co));
	await breathe();

	// Back to home via goto (clean, avoids hidden back buttons)
	await nav('/', HOME);

	return summarize(pages);
}

/**
 * Visit other settings + new-message pages: home → settings → offline →
 * appearance → account → home → new-message → add-contact → home.
 * ~5 page checks.
 */
export async function visitOtherPages(options?: VisitOptions): Promise<VisitResult> {
	const pages: PageResult[] = [];
	const co: CheckOpts = {
		checkDarkMode: options?.checkDarkMode,
		checkRTL: options?.checkRTL,
	};

	progress('other:waitHome');
	await waitFor(HOME, NAV_TIMEOUT);

	// Offline — use content selector (offline-back hidden on desktop)
	progress('other:offline');
	await nav('/settings/offline', S.offline.localMailboxToggle);
	pages.push(runCheck('offline', co));
	await breathe();

	// Appearance — use content selector (appearance-back hidden on desktop)
	progress('other:appearance');
	await nav('/settings/appearance', S.appearance.light);
	pages.push(runCheck('appearance', co));
	await breathe();

	// Notifications — mobile only, use content selector
	progress('other:notifications');
	await nav('/settings/notifications', S.notifications.toggle);
	pages.push(runCheck('notifications', co));
	await breathe();

	// Help — navigate directly (help-back hidden on desktop)
	progress('other:help');
	await nav('/settings/help', S.help.contactUsLink);
	pages.push(runCheck('help', co));
	await breathe();

	// Contact Us — click from help page
	progress('other:contactUs');
	click(S.help.contactUsLink);
	await waitFor(S.contactUs.messageInput, NAV_TIMEOUT);
	pages.push(runCheck('contact-us', co));
	await breathe();

	// Account — navigate directly (account-back hidden on desktop)
	await nav('/settings/account', S.account.deleteItem);
	pages.push(runCheck('account', co));
	await breathe();

	// Back to home
	await nav('/', HOME);
	await breathe();

	progress('other:newMessage');
	click(S.home.newMessageButton);
	await waitFor(S.newMessage.addContact, NAV_TIMEOUT);
	pages.push(runCheck('new-message', co));
	await breathe();

	// Add Contact (from new-message) — use copyButton (always present)
	click(S.newMessage.addContact);
	await waitFor(S.addContact.copyButton, NAV_TIMEOUT);
	pages.push(runCheck('new-message-add-contact', co));
	await breathe();

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

	progress('chat:waitHome');
	await waitFor(HOME, NAV_TIMEOUT);

	if (options?.hasChat) {
		progress('chat:clickChat');
		const chatLink = document.querySelector(`${S.home.chatList} a`) as HTMLElement | null;
		if (chatLink) {
			chatLink.click();
			await waitFor(S.directChat.messages, NAV_TIMEOUT);
			pages.push(runCheck('direct-chat', co));
			await breathe();

			// Chat settings → back (chat-settings-back is always visible)
			click(S.directChat.settingsLink);
			await waitFor(S.chatSettings.back, NAV_TIMEOUT);
			pages.push(runCheck('chat-settings', co));
			await breathe();

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
	progress('visitAllPages:profilePages');
	const profile = await visitProfilePages(options);
	progress('visitAllPages:otherPages');
	const other = await visitOtherPages(options);
	progress('visitAllPages:chatPages');
	const chat = await visitChatPages(options);
	progress('visitAllPages:done');
	const pages = [...profile.pages, ...other.pages, ...chat.pages];
	return summarize(pages);
}
