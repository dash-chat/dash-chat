import type { Agent } from '../../setup/setup-agents';
import { checkPage, type CheckOptions, type PageResult } from './checks';

export interface VisitOptions {
	checkDarkMode?: boolean;
	checkRTL?: boolean;
	/** Include the direct-chat + chat-settings pages (requires a chat to exist). */
	hasChat?: boolean;
}

export interface VisitResult {
	pages: PageResult[];
	summary: { totalIssues: number; pagesVisited: number };
}

/** Yield to the event loop so WebKitGTK can settle before the next big DOM scan. */
function breathe(): Promise<void> {
	return new Promise(r => setTimeout(r, 200));
}

async function runCheck(
	agent: Agent,
	pageName: string,
	options: CheckOptions,
): Promise<PageResult> {
	const result = await checkPage(agent, options);
	return { page: pageName, ...result };
}

function summarize(pages: PageResult[]): VisitResult {
	const totalIssues = pages.reduce((sum, p) => {
		let count = sum + p.overflow.length;
		if (p.darkMode) count += p.darkMode.issues.length;
		if (p.rtl) count += p.rtl.issues.length;
		return count;
	}, 0);
	return { pages, summary: { totalIssues, pagesVisited: pages.length } };
}

/** Visit home → settings → profile → its sub-pages → back to home. */
export async function visitProfilePages(
	agent: Agent,
	options?: VisitOptions,
): Promise<VisitResult> {
	const co: CheckOptions = {
		checkDarkMode: options?.checkDarkMode,
		checkRTL: options?.checkRTL,
	};
	const pages: PageResult[] = [];

	await agent.homePage.ready();
	await breathe();
	pages.push(await runCheck(agent, 'home', co));
	await breathe();

	// Home-with-tooltip only renders when the chat list is empty.
	if (await agent.homePage.emptyState.isExisting()) {
		await agent.execute(() =>
			localStorage.removeItem('first-chat-tooltip-shown'),
		);
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.back.click();
		await agent.homePage.firstChatTooltip.waitForExist();
		pages.push(await runCheck(agent, 'home-with-tooltip', co));
		await agent.homePage.firstChatTooltip.click();
		await breathe();
	}

	await agent.homePage.settingsLink.click();
	await agent.settingsPage.ready();
	pages.push(await runCheck(agent, 'settings', co));
	await breathe();

	await agent.settingsPage.profileLink.click();
	await agent.profilePage.ready();
	pages.push(await runCheck(agent, 'profile', co));
	await breathe();

	await agent.profilePage.editName.click();
	await agent.editNamePage.ready();
	pages.push(await runCheck(agent, 'edit-name', co));
	await breathe();
	await agent.editNamePage.back.click();
	await agent.profilePage.ready();

	await agent.profilePage.editAbout.click();
	await agent.editAboutPage.ready();
	pages.push(await runCheck(agent, 'edit-about', co));
	await breathe();
	await agent.editAboutPage.back.click();
	await agent.profilePage.ready();

	await agent.profilePage.editPhoto.click();
	await agent.editPhotoPage.ready();
	pages.push(await runCheck(agent, 'edit-photo', co));
	await breathe();
	await agent.editPhotoPage.close.click();
	await agent.profilePage.ready();

	await agent.profilePage.qrLink.click();
	await agent.addContactPage.ready();
	pages.push(await runCheck(agent, 'profile-add-contact', co));
	await breathe();

	await agent.addContactPage.back.click();
	await agent.profilePage.ready();
	if (await agent.profilePage.back.isDisplayed()) {
		await agent.profilePage.back.click();
		await agent.settingsPage.ready();
	}
	await agent.settingsPage.back.click();
	await agent.homePage.ready();

	return summarize(pages);
}

/** Visit the other settings + new-message pages. */
export async function visitOtherPages(
	agent: Agent,
	options?: VisitOptions,
): Promise<VisitResult> {
	const co: CheckOptions = {
		checkDarkMode: options?.checkDarkMode,
		checkRTL: options?.checkRTL,
	};
	const pages: PageResult[] = [];

	await agent.homePage.ready();
	await agent.homePage.settingsLink.click();
	await agent.settingsPage.ready();

	await agent.settingsPage.offlineLink.click();
	await agent.offlinePage.ready();
	pages.push(await runCheck(agent, 'offline', co));
	await breathe();
	// In narrow mode the per-page back goes to /settings; in wide-screen it is
	// hidden and the SettingsPanel sidebar links to siblings are mounted.
	if (await agent.offlinePage.back.isDisplayed()) {
		await agent.offlinePage.back.click();
		await agent.settingsPage.ready();
	}

	await agent.settingsPage.appearanceLink.click();
	await agent.appearancePage.ready();
	pages.push(await runCheck(agent, 'appearance', co));
	await breathe();
	if (await agent.appearancePage.back.isDisplayed()) {
		await agent.appearancePage.back.click();
		await agent.settingsPage.ready();
	}

	await agent.settingsPage.notificationsLink.click();
	await agent.notificationsPage.ready();
	pages.push(await runCheck(agent, 'notifications', co));
	await breathe();
	if (await agent.notificationsPage.back.isDisplayed()) {
		await agent.notificationsPage.back.click();
		await agent.settingsPage.ready();
	}

	await agent.settingsPage.helpLink.click();
	await agent.helpPage.ready();
	pages.push(await runCheck(agent, 'help', co));
	await breathe();

	await agent.helpPage.contactUsLink.click();
	await agent.contactUsPage.ready();
	pages.push(await runCheck(agent, 'contact-us', co));
	await breathe();
	await agent.contactUsPage.back.click();
	await agent.helpPage.ready();
	if (await agent.helpPage.back.isDisplayed()) {
		await agent.helpPage.back.click();
		await agent.settingsPage.ready();
	}

	await agent.settingsPage.accountLink.click();
	await agent.accountPage.ready();
	pages.push(await runCheck(agent, 'account', co));
	await breathe();
	if (await agent.accountPage.back.isDisplayed()) {
		await agent.accountPage.back.click();
		await agent.settingsPage.ready();
	}

	await agent.settingsPage.back.click();
	await agent.homePage.ready();
	await breathe();

	await agent.homePage.newMessageButton.click();
	await agent.newMessagePage.ready();
	pages.push(await runCheck(agent, 'new-message', co));
	await breathe();

	await agent.newMessagePage.addContact.click();
	await agent.addContactPage.ready();
	pages.push(await runCheck(agent, 'new-message-add-contact', co));
	await breathe();

	// /new-message/add-contact passes showBack={!isWideScreen} to AddContactPanel,
	// so its back button is only mounted in narrow mode. In wide-screen the
	// NewMessagePanel sidebar back is used directly.
	if (await agent.addContactPage.back.isDisplayed()) {
		await agent.addContactPage.back.click();
		await agent.newMessagePage.ready();
	}
	await agent.newMessagePage.back.click();
	await agent.homePage.ready();

	return summarize(pages);
}

/** Visit home → direct-chat → chat-settings → home (skips if no chat exists). */
export async function visitChatPages(
	agent: Agent,
	options?: VisitOptions,
): Promise<VisitResult> {
	const co: CheckOptions = {
		checkDarkMode: options?.checkDarkMode,
		checkRTL: options?.checkRTL,
	};
	const pages: PageResult[] = [];

	await agent.homePage.ready();
	if (!options?.hasChat) return summarize(pages);

	const firstChat = agent.homePage.chatList.$('a');
	if (!(await firstChat.isExisting())) return summarize(pages);

	await firstChat.click();
	await agent.directChatPage.messages.waitForExist();
	pages.push(await runCheck(agent, 'direct-chat', co));
	await breathe();

	await agent.directChatPage.settingsLink.click();
	await agent.chatSettingsPage.ready();
	pages.push(await runCheck(agent, 'chat-settings', co));
	await breathe();

	await agent.chatSettingsPage.back.click();
	await agent.directChatPage.ready();
	// direct-chat-back is hidden in wide-screen ({#if !isWideScreen}); when
	// present, click it. In wide-screen the sidebar is the home view, so the
	// URL stays on /direct-chats/X but the home content is already mounted.
	if (await agent.directChatPage.back.isDisplayed()) {
		await agent.directChatPage.back.click();
	}
	await agent.homePage.ready();

	return summarize(pages);
}

/** Visit ALL pages (profile + other + chat). */
export async function visitAllPages(
	agent: Agent,
	options?: VisitOptions,
): Promise<VisitResult> {
	const profile = await visitProfilePages(agent, options);
	const other = await visitOtherPages(agent, options);
	const chat = await visitChatPages(agent, options);
	const all = [...profile.pages, ...other.pages, ...chat.pages];
	return summarize(all);
}
