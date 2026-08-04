/**
 * Shared helpers for E2E test setup.
 *
 * `[a, b] = await setupAgents(this, [{ platform: 'any' }, { platform: 'desktop' }])`
 * returns one `Agent` per requirement — each a `WebdriverIO.Browser` plus
 * page-object instances
 * (`agent.homePage`, `agent.directChatPage`, …) and a small set of agent-level
 * helpers that proxy to the browser-side test registry (`agent.tr`,
 * `agent.goto`, `agent.setLocale`, …) — or skips the suite when the PLATFORMS
 * multiset can't fulfill the requirements.
 */
import { PeerProfileSheet } from '../helpers/components/peer-profile-sheet';
import { APP_PACKAGE } from './platforms/android';
import { Toast } from '../helpers/components/toast';
import { UpdaterBanner } from '../helpers/components/updater-banner';
import { CreateProfilePage } from '../helpers/pages/create-profile-page';
import { ChatSettingsPage } from '../helpers/pages/direct-chats/chat-settings-page';
import { DirectChatPage } from '../helpers/pages/direct-chats/direct-chat-page';
import { AddMembersPage } from '../helpers/pages/group-chat/add-members-page';
import { GroupChatPage } from '../helpers/pages/group-chat/group-chat-page';
import { GroupInfoEditPage } from '../helpers/pages/group-chat/group-info-edit-page';
import { GroupInfoPage } from '../helpers/pages/group-chat/group-info-page';
import { HomePage } from '../helpers/pages/home-page';
import { NewGroupPage } from '../helpers/pages/new-group/new-group-page';
import { AddContactPage } from '../helpers/pages/new-message/add-contact-page';
import { NewMessagePage } from '../helpers/pages/new-message/new-message-page';
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
import { checkOverflow } from '../helpers/review/checks';
import { type AgentPlatformName, platformNames } from './test-env';

export type Agent = WebdriverIO.Browser & {
	/** The platform this agent was launched on. */
	platform: AgentPlatformName;

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
	addMembersPage: AddMembersPage;
	groupChatPage: GroupChatPage;
	groupInfoEditPage: GroupInfoEditPage;
	groupInfoPage: GroupInfoPage;
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
	/** Dispatch a URL through the app's deep link routing logic. */
	handleDeepLink(url: string): Promise<void>;
	/** Resolve a paraglide message key in the agent's current locale. */
	tr(key: string, params?: Record<string, unknown>): Promise<string>;
	/** Scan the whole page for horizontal-overflow issues. */
	checkOverflow(): Promise<string[]>;
	/** Force the responsive `isWideScreen` store (true = desktop, false = mobile). */
	setWideScreen(value: boolean): Promise<void>;
	/** Whether this agent's device can legitimately show the wide (two-panel)
	 *  layout: always true on desktop, and true on mobile only when the
	 *  viewport matches the same media query `screen.svelte.ts` uses (tablets,
	 *  not phones). */
	supportsWideScreen(): Promise<boolean>;
	/** Cold-restart the app: relaunch the binary against the same data dir (the
	 *  Rust node re-hydrates from persisted state), re-attach fresh page objects
	 *  to the new session, and restore narrow layout. */
	restart(): Promise<void>;
	/** Close the app, leaving its on-disk state intact so [`startApp`] brings
	 *  the same user back. Android keeps the WebDriver session alive: tearing it
	 *  down there reinstalls the APK on the next session, so the app would
	 *  return as a fresh install with no profile. */
	stopApp(): Promise<void>;
	/** Relaunch after [`stopApp`] and wait until the app is interactive again. */
	startApp(): Promise<void>;
	/** Switch the Konsta theme. */
	setTheme(theme: 'material' | 'ios'): Promise<void>;
	/** Force dark mode on/off via the test event. */
	setDarkMode(value: boolean): Promise<void>;
	/** Enable preview features so gated UI (e.g. new-group) becomes visible. */
	enablePreviewFeatures(): Promise<void>;
	/** Close this agent's iroh endpoint so it can no longer sync over p2p.
	 *  One-way for the life of the process; the agent still reads/writes
	 *  locally and talks to a mailbox. */
	disableP2p(): Promise<void>;
};

/** (Re)build every page object against `b`. Called on first setup and again
 *  after a restart so the new session never reuses stale element ids. */
function attachPages(agent: Agent, b: WebdriverIO.Browser): void {
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
	agent.addMembersPage = new AddMembersPage(b);
	agent.groupChatPage = new GroupChatPage(b);
	agent.groupInfoEditPage = new GroupInfoEditPage(b);
	agent.groupInfoPage = new GroupInfoPage(b);
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
}

export function makeAgent(b: WebdriverIO.Browser): Agent {
	const agent = b as Agent;
	attachPages(agent, b);

	agent.goto = async (path: string) => {
		await b.execute(async (p: string) => {
			await window.__test.goto(p);
		}, path);
	};
	agent.handleDeepLink = async (url: string) => {
		await b.execute((u: string) => window.__test.handleDeepLink(u), url);
	};
	agent.tr = async (key: string, params?: Record<string, unknown>) =>
		await b.execute(
			async (k: string, p: Record<string, unknown> | undefined) => {
				type Key = Parameters<Window['__test']['tr']>[0];
				type Params = Parameters<Window['__test']['tr']>[1];
				return window.__test.tr(k as Key, p as Params);
			},
			key,
			params,
		);
	agent.checkOverflow = async () => checkOverflow(b);
	agent.setWideScreen = async (value: boolean) => {
		await b.execute(
			(v: boolean) =>
				window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: v })),
			value,
		);
	};
	agent.supportsWideScreen = async () =>
		b.execute(() => {
			const mobile = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
			return (
				!mobile ||
				window.matchMedia('(min-width: 768px) and (min-height: 500px)').matches
			);
		});
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
				window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: v })),
			value,
		);
	};
	agent.enablePreviewFeatures = async () => {
		await b.execute(() => window.__test.enablePreviewFeatures());
	};
	agent.disableP2p = async () => {
		await b.executeAsync((done: () => void) =>
			window.__test.disableP2p().then(done, done),
		);
	};
	agent.restart = async () => {
		await b.reloadSession();
		await waitForTestUtils(b);
		attachPages(agent, b);
		await agent.setWideScreen(false);
	};
	agent.stopApp = async () => {
		if (agent.platform === 'desktop') {
			await b.deleteSession();
			return;
		}
		// Leave the webview first: the session drives it through chromedriver, so
		// tearing it down underneath the session invalidates the session itself.
		await b.switchContext('NATIVE_APP');
		await b.terminateApp(APP_PACKAGE);
	};
	agent.startApp = async () => {
		if (agent.platform === 'desktop') {
			await b.reloadSession();
		} else {
			await b.activateApp(APP_PACKAGE);
			// A relaunch drops back to the native context; only the session's
			// initial `autoWebview` does this for us.
			await switchToWebview(b);
		}
		await waitForTestUtils(b);
		attachPages(agent, b);
		await agent.setWideScreen(false);
	};

	return agent;
}

/** Attach to the app's webview context, which a relaunch drops out of. */
async function switchToWebview(agent: WebdriverIO.Browser): Promise<void> {
	let webview: string | undefined;
	await agent.waitUntil(
		async () => {
			const contexts = await agent.getContexts();
			webview = contexts
				.map(context => (typeof context === 'string' ? context : context.id))
				.find(id => id.startsWith('WEBVIEW'));
			return webview !== undefined;
		},
		{ timeoutMsg: 'no WEBVIEW context after relaunch' },
	);
	await agent.switchContext(webview!);
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
async function setupAgent(
	agentName: string,
	platform: AgentPlatformName,
): Promise<Agent> {
	const b = browser.getInstance(agentName);
	await waitForTestUtils(b);
	const agent = makeAgent(b);
	agent.platform = platform;
	await agent.setWideScreen(false);
	return agent;
}

/** What a spec requires of one agent's platform. 'android' is fulfilled by a
 *  physical device or an emulator; 'ios' by a connected iPhone. */
export type PlatformRequirement = 'desktop' | 'android' | 'ios' | 'any';

/** What a spec requires of one agent. */
export interface AgentRequirement {
	platform: PlatformRequirement;
}

function fulfills(
	requirement: PlatformRequirement,
	platform: AgentPlatformName,
): boolean {
	if (requirement === 'any') return true;
	if (requirement === 'desktop') return platform === 'desktop';
	if (requirement === 'android') {
		return platform === 'android' || platform === 'android-emulator';
	}
	if (requirement === 'ios') return platform === 'ios';
	return false;
}

/** Assign each requirement a distinct launched slot — specific requirements
 *  first so 'any' takes the leftovers, ascending slot order for determinism —
 *  or null when the launched platforms can't fulfill them all. */
function matchSlots(
	requirements: readonly PlatformRequirement[],
	platforms: AgentPlatformName[],
): number[] | null {
	const free = platforms.map((platform, i) => ({ slot: i + 1, platform }));
	const slots: number[] = [];
	const order = [...requirements.keys()].sort(
		(a, b) =>
			Number(requirements[a] === 'any') - Number(requirements[b] === 'any'),
	);
	for (const i of order) {
		const j = free.findIndex(f => fulfills(requirements[i], f.platform));
		if (j === -1) return null;
		slots[i] = free[j].slot;
		free.splice(j, 1);
	}
	return slots;
}

/**
 * Build one agent per requirement, matched against the unordered PLATFORMS
 * multiset (a 'desktop' requirement gets a desktop agent no matter its
 * position in PLATFORMS), skipping the suite when no assignment exists. Call
 * from a `before(async function () { ... })` hook (not an arrow function —
 * `this` must be the mocha context so the suite can be skipped).
 */
export async function setupAgents<
	const T extends readonly AgentRequirement[],
>(ctx: Mocha.Context, requirements: T): Promise<{ [K in keyof T]: Agent }> {
	const platforms = platformNames();
	const slots = matchSlots(
		requirements.map(r => r.platform),
		platforms,
	);
	if (slots === null) ctx.skip();
	const agents = await Promise.all(
		slots.map(slot => setupAgent(`agent${slot}`, platforms[slot - 1])),
	);
	return agents as { [K in keyof T]: Agent };
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
