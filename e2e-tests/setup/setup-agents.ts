/**
 * Shared helpers for E2E test setup.
 *
 * `setupAgent('agent1')` returns an `Agent` — a `WebdriverIO.Browser` plus
 * page-object instances (`agent.homePage`, `agent.directChatPage`, …) and a
 * small set of agent-level helpers that proxy to the browser-side test
 * registry (`agent.tr`, `agent.goto`, `agent.setLocale`, …).
 */
import { CreateProfilePage } from '../helpers/pages/create-profile-page';
import { checkOverflow } from '../helpers/review/checks';
import { ChatSettingsPage } from '../helpers/pages/direct-chats/chat-settings-page';
import { DirectChatPage } from '../helpers/pages/direct-chats/direct-chat-page';
import { GroupChatPage } from '../helpers/pages/group-chat/group-chat-page';
import { HomePage } from '../helpers/pages/home-page';
import { NewGroupPage } from '../helpers/pages/new-group/new-group-page';
import { AddContactPage } from '../helpers/pages/new-message/add-contact-page';
import { NewMessagePage } from '../helpers/pages/new-message/new-message-page';
import { PeerProfileSheet } from '../helpers/components/peer-profile-sheet';
import { Toast } from '../helpers/components/toast';
import { AccountPage } from '../helpers/pages/settings/account-page';
import { AppearancePage } from '../helpers/pages/settings/appearance-page';
import { ContactUsPage } from '../helpers/pages/settings/help/contact-us-page';
import { HelpPage } from '../helpers/pages/settings/help/help-page';
import { NotificationsPage } from '../helpers/pages/settings/notifications-page';
import { OfflinePage } from '../helpers/pages/settings/offline-page';
import { EditAboutPage } from '../helpers/pages/settings/profile/edit-about-page';
import { EditNamePage } from '../helpers/pages/settings/profile/edit-name-page';
import { EditPhotoPage } from '../helpers/pages/settings/profile/edit-photo-page';
import { ProfilePage } from '../helpers/pages/settings/profile/profile-page';
import { SettingsPage } from '../helpers/pages/settings/settings-page';
import { UpdaterBanner } from '../helpers/components/updater-banner';

export type Agent = WebdriverIO.Browser & {
	accountPage: AccountPage;
	addContactPage: AddContactPage;
	appearancePage: AppearancePage;
	chatSettingsPage: ChatSettingsPage;
	contactUsPage: ContactUsPage;
	createProfilePage: CreateProfilePage;
	directChatPage: DirectChatPage;
	editAboutPage: EditAboutPage;
	editNamePage: EditNamePage;
	editPhotoPage: EditPhotoPage;
	groupChatPage: GroupChatPage;
	helpPage: HelpPage;
	homePage: HomePage;
	newGroupPage: NewGroupPage;
	newMessagePage: NewMessagePage;
	notificationsPage: NotificationsPage;
	offlinePage: OfflinePage;
	peerProfileSheet: PeerProfileSheet;
	profilePage: ProfilePage;
	settingsPage: SettingsPage;
	toast: Toast;
	updaterBanner: UpdaterBanner;

	/** SvelteKit `goto` — uses `window.__test.goto` for client-side nav. */
	goto(path: string): Promise<void>;
	/** Resolve a paraglide message key in the agent's current locale. */
	tr(key: string): Promise<string>;
	/** Scan the whole page for horizontal-overflow issues. */
	checkOverflow(): Promise<string[]>;
	/** Force the responsive `isWideScreen` store (true = desktop, false = mobile). */
	setWideScreen(value: boolean): Promise<void>;
	/** Switch the Konsta theme. */
	setTheme(theme: 'material' | 'ios'): Promise<void>;
	/** Force dark mode on/off via the test event. */
	setDarkMode(value: boolean): Promise<void>;
	/** Enable preview features so gated UI (e.g. new-group) becomes visible. */
	enablePreviewFeatures(): Promise<void>;
};

export function makeAgent(b: WebdriverIO.Browser): Agent {
	const agent = b as Agent;
	agent.accountPage = new AccountPage(b);
	agent.addContactPage = new AddContactPage(b);
	agent.appearancePage = new AppearancePage(b);
	agent.chatSettingsPage = new ChatSettingsPage(b);
	agent.contactUsPage = new ContactUsPage(b);
	agent.createProfilePage = new CreateProfilePage(b);
	agent.directChatPage = new DirectChatPage(b);
	agent.editAboutPage = new EditAboutPage(b);
	agent.editNamePage = new EditNamePage(b);
	agent.editPhotoPage = new EditPhotoPage(b);
	agent.groupChatPage = new GroupChatPage(b);
	agent.helpPage = new HelpPage(b);
	agent.homePage = new HomePage(b);
	agent.newGroupPage = new NewGroupPage(b);
	agent.newMessagePage = new NewMessagePage(b);
	agent.notificationsPage = new NotificationsPage(b);
	agent.offlinePage = new OfflinePage(b);
	agent.peerProfileSheet = new PeerProfileSheet(b);
	agent.profilePage = new ProfilePage(b);
	agent.settingsPage = new SettingsPage(b);
	agent.toast = new Toast(b);
	agent.updaterBanner = new UpdaterBanner(b);

	agent.goto = async (path: string) => {
		await b.execute(async (p: string) => {
			await window.__test.goto(p);
		}, path);
	};
	agent.tr = async (key: string) =>
		await b.execute(
			async (k: string) =>
				window.__test.tr(k as Parameters<Window['__test']['tr']>[0]),
			key,
		);
	agent.checkOverflow = async () => checkOverflow(b);
	agent.setWideScreen = async (value: boolean) => {
		await b.execute(
			(v: boolean) =>
				window.dispatchEvent(
					new CustomEvent('set-wide-screen', { detail: v }),
				),
			value,
		);
	};
	agent.setTheme = async (theme: 'material' | 'ios') => {
		await b.execute(
			(t: 'material' | 'ios') =>
				window.dispatchEvent(
					new CustomEvent('theme-change', { detail: { theme: t } }),
				),
			theme,
		);
	};
	agent.setDarkMode = async (value: boolean) => {
		await b.execute(
			(v: boolean) =>
				window.dispatchEvent(
					new CustomEvent('set-dark-mode', { detail: v }),
				),
			value,
		);
	};
	agent.enablePreviewFeatures = async () => {
		await b.execute(() => window.__test.enablePreviewFeatures());
	};

	return agent;
}

/** Wait for window.__test to be registered on a single agent. */
export async function waitForTestUtils(
	agent: WebdriverIO.Browser,
): Promise<void> {
	await agent.waitUntil(
		async () => agent.execute(() => typeof window.__test !== 'undefined'),
		{
			timeout: 30_000,
			interval: 500,
			timeoutMsg: 'window.__test not registered',
		},
	);
}

/** Build an agent by capability name and wait for window.__test to be ready.
 *  Defaults to narrow (mobile) layout so back buttons and FABs render — review
 *  checks switch to wide explicitly when they need the desktop two-panel UI. */
export async function setupAgent(agentName: string): Promise<Agent> {
	const b = browser.getInstance(agentName);
	await waitForTestUtils(b);
	const agent = makeAgent(b);
	await agent.setWideScreen(false);
	return agent;
}

/**
 * Switch the agent's UI locale. `window.__test.setLocale` is the overwritten
 * paraglide setLocale that updates the cookie + global-variable strategies
 * without reloading; the layout's `{#key currentLocale}` block re-mounts the
 * rendered route so every `m.foo()` call reads the new locale.
 */
export async function setLocale(agent: Agent, locale: string): Promise<void> {
	await agent.execute((loc: string) => {
		window.__test.setLocale(loc);
	}, locale);
}
