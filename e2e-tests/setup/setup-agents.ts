/**
 * Shared helpers for E2E test setup.
 *
 * `[a, b] = await setupAgents(this, [{ platform: 'any' }, { platform: 'desktop' }])`
 * returns one `Agent` per requirement — each a `WebdriverIO.Browser` plus
 * page-object instances
 * (`agent.homePage`, `agent.directChatPage`, …) and a small set of agent-level
 * helpers that proxy to the browser-side test registry (`agent.tr`,
 * `agent.goto`, `agent.setLocale`, …) — or skips the suite when the PHONES
 * multiset can't fulfill the requirements only a phone can.
 */
import { asyncExitHook } from 'exit-hook';
import { existsSync, readFileSync } from 'node:fs';

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
import { ASYNC_SCRIPT_TIMEOUT } from '../helpers/timeouts';
import { sourceLogFile } from './agent-logger';
import { convergeNetworks, forgetTestNetworks } from './phone-lan';
import {
	APP_PACKAGE,
	androidHasInternet,
	androidWifiInfo,
	denyAndroidNotificationPermission,
	disableAndroidWifi,
	enableAndroidWifi,
	forgetAndroidWifi,
	isAndroidAppRunning,
	joinAndroidWifi,
	leaveAndroidWifi,
	pressAndroidHome,
	resetAndroidNotificationPermission,
	stopAndroidApp,
	waitForAppLinksVerified,
} from './platforms/android';
import {
	clearAgentDir,
	discardDesktopAgent,
	isAgentAppRunning,
	killAgentApp,
	launchAgentApp,
	launchDesktopAgent,
	macWindowRect,
	readOpenedUrls,
} from './platforms/desktop';
import {
	APP_STATE_NOT_RUNNING,
	clearIosAppData,
	iosHasInternet,
	killIosPushExtension,
	resetIosAppState,
} from './platforms/ios';
import {
	disableIosWifi,
	enableIosWifi,
	forgetIosWifi,
	iosWifiInfo,
	joinIosWifi,
	leaveIosWifi,
} from './platforms/ios-wifi';
import {
	type AgentPlatformName,
	type PhonePlatformName,
	isMobile,
	phonePlatforms,
	testNetworkSsids,
} from './test-env';
import { deviceUdid, switchToWebview, waitForTestUtils } from './webview';
import { WIFI_REASSOCIATE_MS, type WifiInfo } from './wifi';

export type Agent = WebdriverIO.Browser & {
	/** The platform this agent was launched on. */
	platform: AgentPlatformName;
	/** Whether the app runs with peer-to-peer connectivity; false once
	 *  `disableP2p` ran, after which it reaches peers through a mailbox only. */
	p2p: boolean;

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
	 *  profile. On desktop this also force-kills any app process left on the
	 *  agent's data dir outside the session (e.g. the instance delete_account
	 *  self-restarts into). */
	stopApp(): Promise<void>;
	/** Send the app to the background (home button) without killing it.
	 *  On Android this is a home-key press; on desktop it is currently a no-op. */
	backgroundApp(): Promise<void>;
	/** Relaunch after [`stopApp`] or [`waitForAppExit`] and wait until the app
	 *  is interactive again. Idempotent: a no-op when the app is already
	 *  running (on mobile it foregrounds a backgrounded app). */
	startApp(): Promise<void>;
	/** Switch the Konsta theme. */
	setTheme(theme: 'material' | 'ios'): Promise<void>;
	/** Force dark mode on/off via the test event. */
	setDarkMode(value: boolean): Promise<void>;
	/** The colour scheme the app currently has applied. */
	getColorScheme(): Promise<'light' | 'dark'>;
	/** Enable preview features so gated UI (e.g. new-group) becomes visible. */
	enablePreviewFeatures(): Promise<void>;
	/** Turn this agent's persisted p2p setting off and rebuild its node without
	 *  peer-to-peer connectivity, so it syncs through mailboxes only. A spec's
	 *  setup step: call it right after `setupAgents`, before the agents meet. */
	disableP2p(): Promise<void>;
	/** The urls this agent asked the OS to open, once at least `count` have
	 *  arrived. Recorded by the harness's `xdg-open` stub, so desktop only. */
	waitForOpenedUrls(count?: number): Promise<string[]>;
	/** Wait until the app process this agent was driving is gone, after an
	 *  action that makes the app shut itself down (today only delete_account).
	 *  Follow with [`startApp`] to get a driveable session again. */
	waitForAppExit(): Promise<void>;
	/** Turn Wi-Fi off, leaving the app foregrounded. Physical phones only:
	 *  Android through adb, with the app on screen throughout; iOS through the
	 *  Settings app, which takes the app off screen for the duration and puts
	 *  it back, as a user changing networks does. Same for turning it on; the
	 *  test networks are handled through the app on iOS. */
	disableWifi(): Promise<void>;
	/** Turn Wi-Fi on and resolve once the device holds a routable IPv4 address
	 *  again, returning it: the supplicant lands on whichever saved network
	 *  scores best, so callers check it is the one they expect. */
	enableWifi(): Promise<string>;
	/** Join the test network `ssid` (an empty `passphrase` means an open
	 *  network) and resolve with the IPv4 address obtained on it. It becomes
	 *  the one test network saved on the device: any other is forgotten once
	 *  the device is on this one, so a killed run can strand a phone on at
	 *  most the network it was on. iOS goes through the app (the
	 *  network-interfaces plugin), with the app on screen throughout; Android
	 *  through adb. */
	joinWifi(ssid: string, passphrase: string): Promise<string>;
	/** Get onto the host's LAN from wherever the device is: radio on, every
	 *  test network forgotten, then every network it lands on that is clearly
	 *  not the host's forgotten in turn (on Android, every other saved network
	 *  too once it is surely on the host's). Resolves with where it settled. */
	leaveWifi(): Promise<WifiInfo>;
	/** Forget every test network without waiting for where the device lands:
	 *  what a move that is about to turn the radio off does first, so nothing
	 *  of the run's is saved while it is off. */
	forgetWifi(): Promise<void>;
	/** Drop and restore Wi-Fi and resolve once the device holds a routable
	 *  IPv4 address again. Physical phones only; throws elsewhere, since no
	 *  other platform can lose its LAN without also losing the driver session.
	 *  Returns the address it came back on so callers can tell a same-network
	 *  reassociation from a jump to a different SSID, which would invalidate
	 *  any discovery measurement taken after it. */
	cycleWifi(downMs: number): Promise<string>;
	/** The network this device is on: its IPv4 address and the LAN it is on,
	 *  '' while it has none, and its SSID where the platform tells an app —
	 *  on iOS only for a test network, which the app added itself; on the
	 *  user's own network it reads ''. On iOS the reading is the app's, so
	 *  it brings the app to the foreground if it is not — call it where that
	 *  is harmless, as with [`hasInternet`]. Same for the test-network operations. */
	wifiInfo(): Promise<WifiInfo>;
	/** Whether the phone can reach the internet. On android this is a pure adb
	 *  probe; on iOS the answer has to come from the app's own webview, so it
	 *  brings the app to the foreground — call it where that is harmless, or
	 *  where what follows resets the app anyway. Physical phones only; throws
	 *  for desktop and for an emulator, which is NAT'd off the host. */
	hasInternet(): Promise<boolean>;
	/** Wipe the app back to first launch and leave it not running, to be
	 *  called between [`stopApp`] and [`startApp`]. On android a `clearApp`
	 *  with its runtime permissions granted again, as a new session's fast
	 *  reset leaves them; on iOS the app's own delete_account, which means the
	 *  app is brought up to run it and exits on its own afterwards; on desktop
	 *  the agent's data directory, which the app is not holding open. */
	clearAppData(): Promise<void>;
	/** Kill the phone's push extension process, so the next push starts a
	 *  fresh one. iOS only. */
	killPushExtension(): Promise<void>;
	/** Take the notification permission back to never asked, so the app's next
	 *  request shows the system dialog. Call it between [`stopApp`] and
	 *  [`startApp`]. Android only. */
	resetNotificationPermission(): void;
	/** Deny the notification permission for good, so the app's requests are
	 *  refused without a dialog. Call it between [`stopApp`] and
	 *  [`startApp`]. Android only. */
	denyNotificationPermission(): void;
	/** What this agent's device has logged so far in the run, as the harness
	 *  captured it. */
	readLog(): string;
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

export function makeAgent(b: WebdriverIO.Browser, slot: number): Agent {
	const agent = b as Agent;
	attachPages(agent, b);

	agent.goto = async (path: string) => {
		await b.execute(async (p: string) => {
			await window.__test.goto(p);
		}, path);
	};
	agent.readLog = () => {
		const file = sourceLogFile(`agent-${slot}`);
		if (!existsSync(file))
			throw new Error(`no log was captured for agent-${slot} at ${file}`);
		return readFileSync(file, 'utf8');
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
			await waitForAppLinksVerified(deviceUdid(b));
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
		// `set_p2p_enabled` rebuilds the node (pause + resume), a few seconds,
		// which the driver's default async-script timeout does not allow for.
		await b.setTimeout({ script: ASYNC_SCRIPT_TIMEOUT });
		await b.executeAsync((done: () => void) =>
			window.__test.disableP2p().then(done, done),
		);
		agent.p2p = false;
	};
	agent.restart = async () => {
		await agent.stopApp();
		await agent.startApp();
		await agent.setWideScreen(false);
	};
	agent.stopApp = async () => {
		if (agent.platform === 'desktop') {
			try {
				await b.deleteSession();
			} catch {
				// The session is already gone when the app shut itself down; the
				// kill below still reaps whatever is left on the data dir.
			}
			await killAgentApp(slot);
			return;
		}
		// Leave the webview first: the session drives it through chromedriver, so
		// tearing it down underneath the session invalidates the session itself.
		await b.switchContext('NATIVE_APP');
		if (agent.platform === 'ios') {
			await b.terminateApp(APP_PACKAGE);
			return;
		}
		stopAndroidApp(deviceUdid(b));
	};
	agent.backgroundApp = async () => {
		if (agent.platform === 'desktop') {
			return;
		}
		if (agent.platform === 'ios') {
			// A real home-button press. `terminateApp` is not a substitute here:
			// XCUITest's terminate reads to iOS as a user force-quit
			await b.execute('mobile: backgroundApp');
			return;
		}
		pressAndroidHome(deviceUdid(b));
		// ProcessLifecycleOwner — which the lifecycle plugin observes — posts its
		// ON_PAUSE/ON_STOP dispatch on a 700ms delay and cancels it outright if an
		// activity resumes first, so it can tell a real backgrounding apart from a
		// configuration change. Returning before that fires would let a following
		// `startApp` cancel the dispatch, leaving the app's Rust `on_pause` hook
		// unrun and the "background" purely notional.
		await b.pause(PROCESS_LIFECYCLE_DISPATCH_MS);
	};
	agent.startApp = async () => {
		if (agent.platform === 'desktop') {
			// A live session means the app is running and driveable, and there
			// is nothing to do: launching again would start a second process on
			// the same data dir.
			try {
				await b.getTitle();
				return;
			} catch {
				// Session gone — the app was stopped; relaunch below.
			}
			await launchAgentApp(slot);
			await b.reloadSession();
		} else {
			// activateApp is itself idempotent: it launches a stopped app and
			// merely foregrounds a running one.
			await b.activateApp(APP_PACKAGE);
			// A relaunch drops back to the native context; only the session's
			// initial `autoWebview` does this for us.
			await switchToWebview(b, agent.platform);
		}
		await waitForTestUtils(b);
		attachPages(agent, b);
		if (agent.platform === 'desktop') await agent.setWideScreen(false);
	};
	agent.disableWifi = async () => {
		if (agent.platform === 'ios') {
			await disableIosWifi(b);
			return;
		}
		await disableAndroidWifi(wifiUdid(agent, b));
	};
	agent.enableWifi = async () =>
		agent.platform === 'ios'
			? await enableIosWifi(b)
			: await enableAndroidWifi(wifiUdid(agent, b));
	const joinWifiOnce = async (ssid: string, passphrase: string) =>
		agent.platform === 'ios'
			? await joinIosWifi(b, ssid, passphrase)
			: await joinAndroidWifi(
					wifiUdid(agent, b),
					ssid,
					passphrase,
					testNetworkSsids(),
				);
	agent.joinWifi = async (ssid: string, passphrase: string) => {
		// A phone sometimes fails to associate with an access point it was just
		// on; one more try keeps that radio hiccup from failing the spec.
		try {
			return await joinWifiOnce(ssid, passphrase);
		} catch (err) {
			console.warn(`failed to join ${ssid}, retrying: ${String(err)}`);
			return await joinWifiOnce(ssid, passphrase);
		}
	};
	agent.leaveWifi = async () =>
		agent.platform === 'ios'
			? await leaveIosWifi(b, testNetworkSsids())
			: await leaveAndroidWifi(wifiUdid(agent, b), testNetworkSsids());
	agent.forgetWifi = async () => {
		if (agent.platform === 'ios') {
			await forgetIosWifi(b);
			return;
		}
		forgetAndroidWifi(wifiUdid(agent, b), testNetworkSsids());
	};
	agent.cycleWifi = async (downMs: number) => {
		await agent.disableWifi();
		await b.pause(downMs);
		return await agent.enableWifi();
	};
	agent.wifiInfo = async () =>
		agent.platform === 'ios'
			? await iosWifiInfo(b)
			: androidWifiInfo(deviceUdid(b));
	agent.hasInternet = async () =>
		agent.platform === 'ios'
			? await iosHasInternet(b)
			: androidHasInternet(wifiUdid(agent, b));
	agent.clearAppData = async () => {
		if (agent.platform === 'ios') {
			await clearIosAppData(b);
			return;
		}
		if (agent.platform === 'desktop') {
			clearAgentDir(slot);
			return;
		}
		if (agent.platform !== 'android' && agent.platform !== 'android-emulator') {
			throw new Error(
				`clearAppData needs a phone or desktop, got ${agent.platform}`,
			);
		}
		await b.execute('mobile: clearApp', { appId: APP_PACKAGE });
		await b.execute('mobile: changePermissions', {
			permissions: 'all',
			appPackage: APP_PACKAGE,
			action: 'grant',
		});
	};
	agent.killPushExtension = async () => {
		if (agent.platform !== 'ios') {
			throw new Error(`only iOS runs a push extension, got ${agent.platform}`);
		}
		killIosPushExtension(deviceUdid(b));
	};
	agent.resetNotificationPermission = () => {
		if (agent.platform !== 'android' && agent.platform !== 'android-emulator') {
			throw new Error(
				`resetNotificationPermission needs android, got ${agent.platform}`,
			);
		}
		resetAndroidNotificationPermission(deviceUdid(b));
	};
	agent.denyNotificationPermission = () => {
		if (agent.platform !== 'android' && agent.platform !== 'android-emulator') {
			throw new Error(
				`denyNotificationPermission needs android, got ${agent.platform}`,
			);
		}
		denyAndroidNotificationPermission(deviceUdid(b));
	};

	return agent;
}

/** The udid behind the adb side of the Wi-Fi controls: a desktop cannot lose
 *  its LAN without also losing the driver session, and an emulator is not on
 *  one. */
function wifiUdid(agent: Agent, b: WebdriverIO.Browser): string {
	if (agent.platform !== 'android') {
		throw new Error(
			`Wi-Fi control needs a physical phone, got ${agent.platform}`,
		);
	}
	return deviceUdid(b);
}

/** Comfortably past ProcessLifecycleOwner's 700ms background-dispatch delay, so
 *  the app is observably backgrounded by the time `backgroundApp` returns. */
const PROCESS_LIFECYCLE_DISPATCH_MS = 1_500;

/** Held long enough that WebKit reads the touch as a tap instead of the start of
 *  a scroll, and well under the app's 500ms long-press threshold. */
const TAP_HOLD_MS = 100;

/** Between the two reads of a tap target's centre that must agree before it
 *  is tapped: a fraction of the app's longest open transition (400ms). */
const TAP_SETTLE_MS = 100;

/** How many times to re-tap an element whose tap never reached the page. */
const TAP_ATTEMPTS = 3;

/** How long a busy page gets to turn a touch into its click before the tap
 *  counts as dropped. Asked any sooner, a click that is merely late reads as
 *  a miss, and the retry waits on an element the first tap has already
 *  navigated away from. */
const CLICK_DISPATCH_MS = 3_000;

/** How often the page is asked whether the click has fired yet. */
const CLICK_POLL_MS = 100;

/** A fresh handle for `element`, resolved again through the same parent chain
 *  it was originally found by, or null if it is no longer in the page. */
async function refetch(
	element: WebdriverIO.Element,
): Promise<WebdriverIO.Element | null> {
	const parent = element.parent;
	const scope =
		'selector' in parent && parent.selector !== undefined
			? await refetch(parent as WebdriverIO.Element)
			: parent;
	if (scope === null) return null;
	const fresh = await scope.$(element.selector).getElement();
	return (await fresh.isExisting()) ? fresh : null;
}

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
): Promise<{ x: number; y: number; live: WebdriverIO.Element }> {
	// Without this the wait below spends its whole timeout re-throwing "not a
	// valid element" from execute, and reports that instead of the real problem:
	// the app is on a different page than the test thinks.
	if (!(await element.isExisting())) {
		throw new Error(
			`Cannot tap ${String(element.selector)}: it is not in the page`,
		);
	}
	// A re-render between resolving the handle and polling it invalidates the
	// handle, and `isExisting()` cannot tell: it re-queries the selector and
	// answers for the replacement node. So the poll re-fetches through the
	// handle's own parent/selector chain each time (which is what WDIO does for
	// stale elements) rather than reusing the one it was given. Selectors here
	// are not all CSS — `a*=name` chains off a parent — so this cannot be a
	// `document.querySelector` inside the page.
	const centreIfTopmost = (el: HTMLElement) => {
		let rect = el.getBoundingClientRect();
		let x = rect.x + rect.width / 2;
		let y = rect.y + rect.height / 2;
		if (x < 0 || y < 0 || x > window.innerWidth || y > window.innerHeight) {
			el.scrollIntoView({ block: 'center', inline: 'center' });
			rect = el.getBoundingClientRect();
			x = rect.x + rect.width / 2;
			y = rect.y + rect.height / 2;
		}
		const topmost = document.elementFromPoint(x, y);
		return topmost !== null && (topmost === el || el.contains(topmost))
			? { x, y }
			: null;
	};
	let replaced = 0;
	const centreOf = async (live: WebdriverIO.Element) => {
		try {
			return await agent.execute(centreIfTopmost, live);
		} catch {
			replaced++;
			return null;
		}
	};
	try {
		return await agent.waitUntil(async () => {
			const live = await refetch(element);
			if (live === null) return null;
			const point = await centreOf(live);
			if (point === null) return null;
			// A menu still scaling or sliding in reports the centre it has now,
			// not the one it settles at, and a tap there lands beside it — on a
			// backdrop that closes the menu. Tap only once the centre holds still.
			await agent.pause(TAP_SETTLE_MS);
			const settled = await centreOf(live);
			if (settled === null || settled.x !== point.x || settled.y !== point.y) {
				return null;
			}
			return { ...point, live };
		});
	} catch (err) {
		const why = err instanceof Error ? err.message : String(err);
		const live = (await refetch(element)) ?? element;
		throw new Error(
			`${String(element.selector)} is in the page but never became the ` +
				'topmost element at its own centre, so a tap there would have hit ' +
				`${await describeCover(agent, live)}; it was replaced under the ` +
				`check ${replaced} times (${why})`,
		);
	}
}

/** What a tap at `element`'s centre would have hit instead of it. A cover is
 *  often invisible — a backdrop a popover left behind at opacity 0 is in no
 *  screenshot — so the failure has to name it rather than point at it. */
async function describeCover(
	agent: WebdriverIO.Browser,
	element: WebdriverIO.Element,
): Promise<string> {
	const describe = (el: HTMLElement) => {
		const rect = el.getBoundingClientRect();
		const top = document.elementFromPoint(
			rect.x + rect.width / 2,
			rect.y + rect.height / 2,
		);
		if (top === null) return 'nothing: its centre is outside the viewport';
		const style = window.getComputedStyle(top);
		const testid = top.getAttribute('data-testid');
		const klass = top.getAttribute('class');
		const names = klass === null ? '' : klass.trim().split(/\s+/).join('.');
		return [
			top.tagName.toLowerCase(),
			testid === null ? '' : `[data-testid="${testid}"]`,
			names === '' ? '' : `.${names}`,
			` (${style.position}, opacity ${style.opacity}, z-index ${style.zIndex})`,
		].join('');
	};
	try {
		return await agent.execute(describe, element);
	} catch {
		return 'something the page replaced before it could be named';
	}
}

/** Whether `element` cannot be clicked: a disabled control, which gets no
 *  click at all. */
function isDisabled(
	agent: WebdriverIO.Browser,
	element: WebdriverIO.Element,
): Promise<boolean> {
	return agent.execute(
		(el: HTMLElement) =>
			el.matches(':disabled') || el.getAttribute('aria-disabled') === 'true',
		element,
	);
}

/** Whether a tap whose click did not reach `element` still did what a click
 *  on it does. Some touches act without the click ever reaching a listener, or
 *  with it arriving after the check, and a retry then taps something else.
 *  Two outcomes count: the element turned disabled since before the first tap
 *  (a send button busy sending), which no click elsewhere can cause; or, when
 *  no click was dispatched at all, it left the page (a back button, as its page
 *  goes) — a click elsewhere, on a backdrop, could have removed it too. */
async function tapTookEffect(
	agent: WebdriverIO.Browser,
	element: WebdriverIO.Element,
	disabledAtStart: boolean,
	click: 'elsewhere' | 'none',
): Promise<boolean> {
	const live = await refetch(element);
	if (live === null) return click === 'none';
	if (disabledAtStart) return false;
	try {
		return await isDisabled(agent, live);
	} catch {
		return false;
	}
}

/** Touch (x, y) and report where the click it dispatched landed: on
 *  `element`, elsewhere, or nowhere.
 *
 *  WDA reports a successful touch that WebKit sometimes never turns into a
 *  click, so the tap has to be confirmed rather than assumed. It has to be
 *  confirmed *on the target*: the point is checked before the touch and the
 *  round trip takes most of a second, so a page that moves in between leaves
 *  the tap landing on something else — which still fires a click, just not the
 *  one that was asked for. The flag lives on documentElement because a click
 *  that lands usually starts a navigation and takes the element with it, and
 *  it records a click anywhere so that the wait for it ends on the first one. */
async function clickReachedElement(
	agent: WebdriverIO.Browser,
	element: WebdriverIO.Element,
	x: number,
	y: number,
	pointerType: PointerType,
): Promise<'target' | 'elsewhere' | 'none'> {
	await agent.execute((el: HTMLElement) => {
		delete document.documentElement.dataset.e2eClick;
		delete document.documentElement.dataset.e2eClickElsewhere;
		document.addEventListener(
			'click',
			event => {
				const target = event.target;
				// A re-render between the hit test and the click hands it to a
				// fresh copy of the same control.
				const testid = el.getAttribute('data-testid');
				const onTarget =
					target instanceof Element &&
					(el.contains(target) ||
						(testid !== null &&
							target.closest(`[data-testid="${testid}"]`) !== null));
				// A click that reaches no element still in the page was dispatched
				// at a target the touch already removed: it acted, it did not miss.
				const elsewhere =
					!onTarget && target instanceof Element && target.isConnected;
				document.documentElement.dataset.e2eClick = onTarget
					? 'target'
					: elsewhere
						? 'elsewhere'
						: 'none';
				if (elsewhere) {
					const testid = target.closest('[data-testid]');
					document.documentElement.dataset.e2eClickElsewhere = [
						target.tagName.toLowerCase(),
						testid === null
							? ''
							: `in [data-testid="${testid.getAttribute('data-testid')}"]`,
					].join(' ');
				}
			},
			{ once: true, capture: true },
		);
	}, element);
	await agent
		.action('pointer', { parameters: { pointerType } })
		.move({ x: Math.round(x), y: Math.round(y) })
		.down()
		.pause(TAP_HOLD_MS)
		.up()
		.perform();
	const tappedAt = Date.now();
	let polls = 0;
	let landed: string | undefined;
	try {
		await agent.waitUntil(
			async () => {
				polls++;
				landed = await agent.execute(
					() => document.documentElement.dataset.e2eClick,
				);
				return landed !== undefined;
			},
			{ timeout: CLICK_DISPATCH_MS, interval: CLICK_POLL_MS },
		);
	} catch {
		console.log(`[tap-timing] no click in ${Date.now() - tappedAt}ms`);
		return 'none';
	}
	if (polls > 1 || landed !== 'target') {
		console.log(
			`[tap-timing] ${landed} on poll ${polls}, ${Date.now() - tappedAt}ms after the touch`,
		);
	}
	return landed === 'target' || landed === 'elsewhere' ? landed : 'none';
}

type PointerType = 'touch' | 'mouse';

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
 *  XCUICoordinate tap, which WebKit drops inside a scrolling container.
 *
 *  A desktop agent needs the same, with a mouse: the app's embedded
 *  WebDriver implements element click as `el.click()` on the element itself,
 *  which never reaches a handler on a child (a Konsta list item's link), while
 *  a pointer action is dispatched at the point's innermost element and
 *  bubbles up like a real click. */
export function tapWebElementsAtTheirRect(
	agent: WebdriverIO.Browser,
	pointerType: PointerType,
): void {
	agent.overwriteCommand(
		'click',
		async function (this: WebdriverIO.Element, origClick) {
			if (pointerType === 'touch') {
				const context = await agent.getContext();
				if (typeof context !== 'string' || !context.startsWith('WEBVIEW')) {
					return await origClick();
				}
			}
			let disabledAtStart: boolean | undefined;
			for (let attempt = 1; attempt <= TAP_ATTEMPTS; attempt++) {
				const { x, y, live } = await tapPoint(agent, this);
				disabledAtStart ??= await isDisabled(agent, live);
				const click = await clickReachedElement(agent, live, x, y, pointerType);
				if (click === 'target') return;
				if (await tapTookEffect(agent, this, disabledAtStart, click)) return;
				const landedOn =
					click === 'elsewhere'
						? await agent.execute(
								() => document.documentElement.dataset.e2eClickElsewhere,
							)
						: 'nothing';
				console.warn(
					`[${pointerType}] tap at ${x},${y} did not reach ${String(this.selector)}; ` +
						`its click went to ${landedOn} (attempt ${attempt}/${TAP_ATTEMPTS})`,
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

/** Tap web elements with a touch action instead of chromedriver's click, which
 *  spends about ten devtools round trips over USB (~800ms on a phone) where the
 *  action needs two. */
function tapWebElementsWithTouch(agent: WebdriverIO.Browser): void {
	agent.overwriteCommand(
		'click',
		async function (this: WebdriverIO.Element, origClick) {
			const context = await agent.getContext();
			if (typeof context !== 'string' || !context.startsWith('WEBVIEW')) {
				return await origClick();
			}
			const { x, y } = await tapPoint(agent, this);
			await agent
				.action('pointer', { parameters: { pointerType: 'touch' } })
				.move({ x: Math.round(x), y: Math.round(y) })
				.down()
				.up()
				.perform();
		},
		true,
	);
}

/** Make deleting a desktop agent's session a no-op once its app is gone. The
 *  WebDriver server runs inside the app, so the session ended with the
 *  process, and a delete sent to the closed port fails instead. */
function skipSessionDeleteOnceAppIsGone(
	agent: WebdriverIO.Browser,
	slot: number,
): void {
	agent.overwriteCommand(
		// @ts-expect-error The typings only accept webdriverio's own commands,
		// but a WebDriver protocol command overwrites the same way.
		'deleteSession',
		async (
			origDeleteSession: WebdriverIO.Browser['deleteSession'],
			...args: Parameters<WebdriverIO.Browser['deleteSession']>
		) => {
			if (!(await isAgentAppRunning(slot))) return;
			await origDeleteSession(...args);
		},
	);
}

/** Build an agent on session `b` and wait for window.__test to be ready.
 *  Defaults to narrow (mobile) layout so back buttons and FABs render — review
 *  checks switch to wide explicitly when they need the desktop two-panel UI. */
async function setupAgent(
	b: WebdriverIO.Browser,
	platform: AgentPlatformName,
	slot: number,
): Promise<Agent> {
	await waitForTestUtils(b);
	if (platform === 'ios') {
		// Each spec file gets fresh sessions but not a fresh install, so state
		// from the previous spec is wiped here.
		await resetIosAppState(b);
		// Before makeAgent: it resolves every page object's element, and an
		// element built before the overwrite keeps the original click.
		tapWebElementsAtTheirRect(b, 'touch');
	} else if (platform === 'desktop') {
		tapWebElementsAtTheirRect(b, 'mouse');
		skipSessionDeleteOnceAppIsGone(b, slot);
		if (process.platform === 'darwin') {
			const { x, y, width, height } = macWindowRect(slot);
			await b.setWindowRect(x, y, width, height);
		}
	} else {
		tapWebElementsWithTouch(b);
	}
	const agent = makeAgent(b, slot);
	agent.platform = platform;
	agent.p2p = true;
	agent.waitForAppExit = async () => {
		if (platform === 'desktop') {
			// The session breaking is the exit signal: the WebDriver server lives
			// in the app, so it goes when the app does.
			await b.waitUntil(
				async () => {
					try {
						await b.getTitle();
						return false;
					} catch {
						return true;
					}
				},
				{ timeoutMsg: 'the app never shut itself down' },
			);
			return;
		}
		await b.switchContext('NATIVE_APP');
		if (platform === 'android') {
			const udid = deviceUdid(b);
			await b.waitUntil(async () => !isAndroidAppRunning(udid), {
				timeoutMsg: 'the app never shut itself down',
			});
			return;
		}
		await b.waitUntil(
			async () =>
				Number(await b.queryAppState(APP_PACKAGE)) <= APP_STATE_NOT_RUNNING,
			{ timeoutMsg: 'the app never shut itself down' },
		);
	};
	agent.waitForOpenedUrls = async (count = 1) => {
		let urls: string[] = [];
		await b.waitUntil(
			() => {
				urls = readOpenedUrls(slot);
				return urls.length >= count;
			},
			{ timeoutMsg: `agent${slot} never asked the OS to open ${count} url(s)` },
		);
		return urls;
	};
	await agent.setWideScreen(false);
	return agent;
}

/** What a spec requires of one agent's platform. 'android' is fulfilled by a
 *  physical device or an emulator; 'ios' by a connected iPhone; 'phone' by any
 *  physical handset, which an emulator is not — it is NAT'd off the host, so
 *  no test network can reach it; 'mobile' by any of those; 'any' by any
 *  launched platform. */
export type PlatformRequirement =
	| 'desktop'
	| 'android'
	| 'ios'
	| 'phone'
	| 'mobile'
	| 'any';

/** What a spec requires of one agent. */
export interface AgentRequirement {
	platform: PlatformRequirement;
}

function fulfills(
	requirement: PlatformRequirement,
	platform: AgentPlatformName,
): boolean {
	if (requirement === 'any') return true;
	if (requirement === 'mobile') return isMobile(platform);
	if (requirement === 'phone') {
		return isMobile(platform) && platform !== 'android-emulator';
	}
	if (requirement === 'desktop') return platform === 'desktop';
	if (requirement === 'android') {
		return platform === 'android' || platform === 'android-emulator';
	}
	if (requirement === 'ios') return platform === 'ios';
	return false;
}

/** How narrow a requirement is: exact platform > 'phone' > 'mobile' > 'any'.
 *  Match the narrowest first so a broad requirement never steals the only slot
 *  a narrow one could have used. */
function specificity(requirement: PlatformRequirement): number {
	if (requirement === 'any') return 0;
	if (requirement === 'mobile') return 1;
	if (requirement === 'phone') return 2;
	return 3;
}

/** Assign each requirement a distinct phone slot when a phone fulfills it,
 *  or null for a desktop agent — narrowest requirements first so broader ones
 *  take the leftover phones, ascending slot order for determinism — or null
 *  overall when a requirement only a phone fulfills finds none. */
function assignPhones(
	requirements: readonly PlatformRequirement[],
	phones: PhonePlatformName[],
): (number | null)[] | null {
	const free = phones.map((platform, i) => ({ slot: i + 1, platform }));
	const slots: (number | null)[] = [];
	const order = [...requirements.keys()].sort(
		(a, b) => specificity(requirements[b]) - specificity(requirements[a]),
	);
	for (const i of order) {
		const j = free.findIndex(f => fulfills(requirements[i], f.platform));
		if (j !== -1) {
			slots[i] = free[j].slot;
			free.splice(j, 1);
		} else if (fulfills(requirements[i], 'desktop')) {
			slots[i] = null;
		} else {
			return null;
		}
	}
	return slots;
}

let desktopsLaunched = 0;

/** Launch one more desktop agent, on the next slot past every phone's. */
export async function setupDesktopAgent(): Promise<{
	agent: Agent;
	slot: number;
}> {
	desktopsLaunched += 1;
	const slot = phonePlatforms().length + desktopsLaunched;
	try {
		const agent = await setupAgent(
			await launchDesktopAgent(slot),
			'desktop',
			slot,
		);
		return { agent, slot };
	} catch (e) {
		// A leftover app would keep advertising over mDNS and skew the rest of the run.
		await discardDesktopAgent(slot);
		throw e;
	}
}

/** The agents `setupAgents` built for this spec file. */
export const specAgents: Agent[] = [];

/** Room for every phone to work through every network it has to forget on
 *  the way back, each waited out up to WIFI_REASSOCIATE_MS. */
const PHONES_BACK_MS = 5 * WIFI_REASSOCIATE_MS;

/** The exit codes exit-hook reports for SIGINT and SIGTERM. */
const SIGNAL_EXIT_CODES = [130, 143];

let signalHookRegistered = false;

/** On Ctrl-C the suite's afterAll never runs, and wdio kills the worker 5s
 *  later: time to forget the test networks, not to wait for the phones to
 *  land. An iPhone left on one cannot verify its developer certificate
 *  offline, and then no session, and so no harness, can reach it again. The
 *  hook joins wdio's own exit-hook, which waits for it; registered in the
 *  worker only, since a launcher hook would end the launcher at once. */
function forgetTestNetworksOnSignal(): void {
	if (signalHookRegistered) return;
	signalHookRegistered = true;
	asyncExitHook(
		async exitCode => {
			if (SIGNAL_EXIT_CODES.includes(exitCode)) {
				await forgetTestNetworks(specAgents);
			}
		},
		{ wait: 4_500 },
	);
}

/**
 * Build one agent per requirement: on a phone from the unordered PHONES
 * multiset when one fulfills it, on a desktop app launched for it otherwise.
 * Skips the suite when a requirement only a phone fulfills finds none. Call
 * from a `before(async function () { ... })` hook (not an arrow function —
 * `this` must be the mocha context so the suite can be skipped).
 */
export async function setupAgents<const T extends readonly AgentRequirement[]>(
	ctx: Mocha.Context,
	requirements: T,
): Promise<{ [K in keyof T]: Agent }> {
	const phones = phonePlatforms();
	const slots = assignPhones(
		requirements.map(r => r.platform),
		phones,
	);
	if (slots === null) ctx.skip();
	const agents = await Promise.all(
		slots.map(async slot =>
			slot === null
				? (await setupDesktopAgent()).agent
				: await setupAgent(
						browser.getInstance(`agent${slot}`),
						phones[slot - 1],
						slot,
					),
		),
	);
	specAgents.push(...agents);
	forgetTestNetworksOnSignal();
	await convergeNetworks(agents);
	// However the suite ends, its phones go back on the host's LAN: an iPhone
	// left on a test network would get the next run's build installed with no
	// internet to verify it.
	ctx.test?.parent?.afterAll(
		'put the phones back on the host LAN',
		function (this: Mocha.Context) {
			this.timeout(PHONES_BACK_MS);
			return convergeNetworks(agents);
		},
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
