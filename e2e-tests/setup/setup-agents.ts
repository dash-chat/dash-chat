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
import { execSync } from 'node:child_process';

import { PeerProfileSheet } from '../helpers/components/peer-profile-sheet';
import { Toast } from '../helpers/components/toast';
import { UpdaterBanner } from '../helpers/components/updater-banner';
import { CreateProfilePage } from '../helpers/pages/create-profile-page';
import { ChatSettingsPage } from '../helpers/pages/direct-chats/chat-settings-page';
import { DirectChatPage } from '../helpers/pages/direct-chats/direct-chat-page';
import { EulaPage } from '../helpers/pages/eula-page';
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
import { WelcomePage } from '../helpers/pages/welcome-page';
import { checkOverflow } from '../helpers/review/checks';
import {
	APP_PACKAGE,
	stopAndroidApp,
	waitForAppLinksVerified,
} from './platforms/android';
import { readOpenedUrls } from './platforms/desktop';
import { resetIosAppState } from './platforms/ios';
import { type AgentPlatformName, platformNames } from './test-env';
import { switchToWebview, waitForTestUtils } from './webview';

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
	eulaPage: EulaPage;
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
	welcomePage: WelcomePage;

	/** SvelteKit `goto` — uses `window.__test.goto` for client-side nav. */
	goto(path: string): Promise<void>;
	/** Deliver a deep link the way the OS would: a real VIEW intent on Android,
	 *  `mobile: deepLink` on iOS. Desktop e2e builds skip the single-instance
	 *  plugin — the OS delivery path for runtime deep links — so there it falls
	 *  back to [`injectDeepLink`]. */
	handleDeepLink(url: string): Promise<void>;
	/** Dispatch a URL through the app's deep link routing logic directly,
	 *  bypassing OS delivery — for links the OS wouldn't route to the app
	 *  (e.g. the dash-chat:// scheme is only registered on desktop). */
	injectDeepLink(url: string): Promise<void>;
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
	 *  the same user back. Android keeps the WebDriver session alive: a new
	 *  session there fast-resets (`pm clear`), so the app would return with no
	 *  profile. */
	stopApp(): Promise<void>;
	/** Send the app to the background (home button) without killing it.
	 *  On Android this is a home-key press; on desktop it is currently a no-op. */
	backgroundApp(): Promise<void>;
	/** Relaunch after [`stopApp`] and wait until the app is interactive again. */
	startApp(): Promise<void>;
	/** Switch the Konsta theme. */
	setTheme(theme: 'material' | 'ios'): Promise<void>;
	/** Force dark mode on/off via the test event. */
	setDarkMode(value: boolean): Promise<void>;
	/** The colour scheme the app currently has applied. */
	getColorScheme(): Promise<'light' | 'dark'>;
	/** Enable preview features so gated UI (e.g. new-group) becomes visible. */
	enablePreviewFeatures(): Promise<void>;
	/** Close this agent's iroh endpoint so it can no longer sync over p2p.
	 *  One-way for the life of the process; the agent still reads/writes
	 *  locally and talks to a mailbox. */
	disableP2p(): Promise<void>;
	/** The urls this agent asked the OS to open, once at least `count` have
	 *  arrived. Recorded by the harness's `xdg-open` stub, so desktop only. */
	waitForOpenedUrls(count?: number): Promise<string[]>;
};

/** The device serial this Appium session was launched against. */
function androidUdid(b: WebdriverIO.Browser): string {
	const udid = b.requestedCapabilities['appium:udid'];
	if (udid === undefined) {
		throw new Error('Android session is missing its appium:udid capability');
	}
	return udid;
}

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
	agent.eulaPage = new EulaPage(b);
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
	agent.welcomePage = new WelcomePage(b);
}

export function makeAgent(b: WebdriverIO.Browser): Agent {
	const agent = b as Agent;
	attachPages(agent, b);

	agent.goto = async (path: string) => {
		await b.execute(async (p: string) => {
			await window.__test.goto(p);
		}, path);
	};
	agent.injectDeepLink = async (url: string) => {
		await b.execute((u: string) => window.__test.handleDeepLink(u), url);
	};
	agent.handleDeepLink = async (url: string) => {
		if (agent.platform === 'desktop') {
			await agent.injectDeepLink(url);
		} else if (agent.platform === 'ios') {
			// Unlike Android's pm get-app-links, the device's AASA validation
			// state can't be asserted from the harness, and a failed validation
			// would open Safari — so route the URL to the app explicitly.
			await b.execute('mobile: deepLink', { url, bundleId: APP_PACKAGE });
		} else {
			// No package pin: the OS resolves the link itself, so this covers the
			// verified App Links association, not just the intent filter. No
			// waitForLaunch: `am start -W` can block forever on a cold launch;
			// callers already wait for the app via page ready()/startApp().
			await waitForAppLinksVerified(androidUdid(b));
			await b.execute('mobile: deepLink', { url, waitForLaunch: false });
		}
	};
	agent.tr = async (key: string, params?: Record<string, unknown>) =>
		await b.execute(
			(k: string, p: Record<string, unknown> | undefined) => {
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
	agent.getColorScheme = () =>
		b.execute(() =>
			document.documentElement.classList.contains('dark') ? 'dark' : 'light',
		);
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
		if (agent.platform === 'ios') {
			await b.terminateApp(APP_PACKAGE);
			return;
		}
		stopAndroidApp(androidUdid(b));
	};
	agent.backgroundApp = async () => {
		if (agent.platform === 'desktop') {
			return;
		}
		if (agent.platform === 'ios') {
			await b.terminateApp(APP_PACKAGE);
			return;
		}
		// Home button press: keeps the process alive so the background service
		// can continue syncing, unlike am stop-app which tears the app down.
		execSync(`adb -s ${androidUdid(b)} shell input keyevent KEYCODE_HOME`, {
			stdio: 'ignore',
		});
	};
	agent.startApp = async () => {
		if (agent.platform === 'desktop') {
			await b.reloadSession();
		} else {
			await b.activateApp(APP_PACKAGE);
			// A relaunch drops back to the native context; only the session's
			// initial `autoWebview` does this for us.
			await switchToWebview(b, agent.platform);
		}
		await waitForTestUtils(b);
		attachPages(agent, b);
		if (agent.platform === 'desktop') await agent.setWideScreen(false);
	};

	return agent;
}

/** Held long enough that WebKit reads the touch as a tap instead of the start of
 *  a scroll, and well under the app's 500ms long-press threshold. */
const TAP_HOLD_MS = 100;

/** How many times to re-tap an element whose tap never reached the page. */
const TAP_ATTEMPTS = 3;

/** The centre of `element`, once a touch there would actually reach it.
 *
 *  A tap is aimed at a point, so it hits whatever is topmost there — during a
 *  page transition that is still the outgoing page, and the tap is swallowed
 *  with the target sitting at exactly the right coordinates. `elementFromPoint`
 *  is the same hit test WebKit will do, so waiting on it makes the tap
 *  self-verifying rather than hoping the transition has finished. */
async function tapPoint(
	agent: WebdriverIO.Browser,
	element: WebdriverIO.Element,
): Promise<{ x: number; y: number }> {
	// Without this the wait below spends its whole timeout re-throwing "not a
	// valid element" from execute, and reports that instead of the real problem:
	// the app is on a different page than the test thinks.
	if (!(await element.isExisting())) {
		throw new Error(
			`Cannot tap ${String(element.selector)}: it is not in the page`,
		);
	}
	return await agent.waitUntil(
		async () =>
			await agent.execute((el: HTMLElement) => {
				const rect = el.getBoundingClientRect();
				const x = rect.x + rect.width / 2;
				const y = rect.y + rect.height / 2;
				const topmost = document.elementFromPoint(x, y);
				return topmost !== null && (topmost === el || el.contains(topmost))
					? { x, y }
					: null;
			}, element),
		{
			timeoutMsg:
				`${String(element.selector)} is in the page but never became the ` +
				'topmost element at its own centre, so a tap there would have hit ' +
				'whatever is covering it',
		},
	);
}

/** Touch (x, y) and report whether `element` actually received a click.
 *
 *  WDA reports a successful touch that WebKit sometimes never turns into a
 *  click, so the tap has to be confirmed rather than assumed. It has to be
 *  confirmed *on the target*: the point is checked before the touch and the
 *  round trip takes most of a second, so a page that moves in between leaves
 *  the tap landing on something else — which still fires a click, just not the
 *  one that was asked for. The flag lives on documentElement because a click
 *  that lands usually starts a navigation and takes the element with it. */
async function clickReachedElement(
	agent: WebdriverIO.Browser,
	element: WebdriverIO.Element,
	x: number,
	y: number,
): Promise<boolean> {
	await agent.execute((el: HTMLElement) => {
		delete document.documentElement.dataset.e2eClick;
		document.addEventListener(
			'click',
			event => {
				const target = event.target;
				if (target instanceof Node && (el === target || el.contains(target))) {
					document.documentElement.dataset.e2eClick = 'seen';
				}
			},
			{ once: true, capture: true },
		);
	}, element);
	await agent
		.action('pointer', { parameters: { pointerType: 'touch' } })
		.move({ x: Math.round(x), y: Math.round(y) })
		.down()
		.pause(TAP_HOLD_MS)
		.up()
		.perform();
	return await agent.execute(
		() => document.documentElement.dataset.e2eClick === 'seen',
	);
}

/** Make webview clicks tap the element's own on-screen rect.
 *
 *  XCUITest's `nativeWebTap` taps the native accessibility element matching the
 *  web element's text, and falls back to a web→native coordinate translation
 *  whenever there is no text (icon-only buttons) or the text matches several
 *  native elements (a confirm dialog over the row that opened it). That
 *  fallback assumes Safari's browser chrome — it insets by a URL bar and scales
 *  the viewport onto a shorter "real" area — so in this app's full-screen
 *  webview it lands tens of points off and the tap silently misses. CSS pixels
 *  here already are screen points, so the rect is the tap point. A pointer
 *  action rather than `mobile: tap`: that one is an instantaneous
 *  XCUICoordinate tap, which WebKit drops inside a scrolling container. */
function tapWebElementsAtTheirRect(agent: WebdriverIO.Browser): void {
	agent.overwriteCommand(
		'click',
		async function (this: WebdriverIO.Element, origClick) {
			const context = await agent.getContext();
			if (typeof context !== 'string' || !context.startsWith('WEBVIEW')) {
				return await origClick();
			}
			for (let attempt = 1; attempt <= TAP_ATTEMPTS; attempt++) {
				const { x, y } = await tapPoint(agent, this);
				if (await clickReachedElement(agent, this, x, y)) return;
				console.warn(
					`[ios] tap at ${x},${y} did not reach ${String(this.selector)} ` +
						`(attempt ${attempt}/${TAP_ATTEMPTS})`,
				);
			}
			throw new Error(
				`tapped ${String(this.selector)} ${TAP_ATTEMPTS} times and it never ` +
					'received a click',
			);
		},
		true,
	);
}

/** Build an agent by capability name and wait for window.__test to be ready.
 *  Defaults to narrow (mobile) layout so back buttons and FABs render — review
 *  checks switch to wide explicitly when they need the desktop two-panel UI. */
async function setupAgent(
	agentName: string,
	platform: AgentPlatformName,
	slot: number,
): Promise<Agent> {
	const b = browser.getInstance(agentName);
	await waitForTestUtils(b);
	if (platform === 'ios') {
		// Each spec file gets fresh sessions but not a fresh install, so state
		// from the previous spec is wiped here.
		await resetIosAppState(b);
		// Before makeAgent: it resolves every page object's element, and an
		// element built before the overwrite keeps the original click.
		tapWebElementsAtTheirRect(b);
	}
	const agent = makeAgent(b);
	agent.platform = platform;
	agent.waitForOpenedUrls = async (count = 1) => {
		let urls: string[] = [];
		await b.waitUntil(
			() => {
				urls = readOpenedUrls(slot);
				return urls.length >= count;
			},
			{ timeoutMsg: `${agentName} never asked the OS to open ${count} url(s)` },
		);
		return urls;
	};
	await agent.setWideScreen(false);
	return agent;
}

/** What a spec requires of one agent's platform. 'android' is fulfilled by a
 *  physical device or an emulator; 'ios' by a connected iPhone; 'mobile' by any
 *  of those (an iOS or Android device); 'any' by any launched platform. */
export type PlatformRequirement =
	| 'desktop'
	| 'android'
	| 'ios'
	| 'mobile'
	| 'any';

/** What a spec requires of one agent. */
export interface AgentRequirement {
	platform: PlatformRequirement;
}

function isMobile(platform: AgentPlatformName): boolean {
	return (
		platform === 'ios' ||
		platform === 'android' ||
		platform === 'android-emulator'
	);
}

function fulfills(
	requirement: PlatformRequirement,
	platform: AgentPlatformName,
): boolean {
	if (requirement === 'any') return true;
	if (requirement === 'mobile') return isMobile(platform);
	if (requirement === 'desktop') return platform === 'desktop';
	if (requirement === 'android') {
		return platform === 'android' || platform === 'android-emulator';
	}
	if (requirement === 'ios') return platform === 'ios';
	return false;
}

/** How narrow a requirement is: exact platform > 'mobile' > 'any'. Match the
 *  narrowest first so a broad requirement never steals the only slot a narrow
 *  one could have used. */
function specificity(requirement: PlatformRequirement): number {
	if (requirement === 'any') return 0;
	if (requirement === 'mobile') return 1;
	return 2;
}

/** Assign each requirement a distinct launched slot — narrowest requirements
 *  first so broader ones take the leftovers, ascending slot order for
 *  determinism — or null when the launched platforms can't fulfill them all. */
function matchSlots(
	requirements: readonly PlatformRequirement[],
	platforms: AgentPlatformName[],
): number[] | null {
	const free = platforms.map((platform, i) => ({ slot: i + 1, platform }));
	const slots: number[] = [];
	const order = [...requirements.keys()].sort(
		(a, b) => specificity(requirements[b]) - specificity(requirements[a]),
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
export async function setupAgents<const T extends readonly AgentRequirement[]>(
	ctx: Mocha.Context,
	requirements: T,
): Promise<{ [K in keyof T]: Agent }> {
	const platforms = platformNames();
	const slots = matchSlots(
		requirements.map(r => r.platform),
		platforms,
	);
	if (slots === null) ctx.skip();
	const agents = await Promise.all(
		slots.map(slot => setupAgent(`agent${slot}`, platforms[slot - 1], slot)),
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
