/**
 * Wi-Fi control for a physical iPhone. Lab networks — the ones a run joins,
 * leaves and forgets — go through the app itself: the network-interfaces
 * plugin's `wifi-control` commands (`NEHotspotConfiguration`), reached over
 * `window.__test.wifi`, which join and forget in seconds with the app on
 * screen throughout and list exactly the networks the app added, in range or
 * not. The radio is the one thing iOS gives no app an API for, so turning it
 * off and on is driven through the Settings app over the XCUITest session,
 * tapping what a user would: that takes the app off screen for the duration
 * and puts it back, as a user changing networks does, while the session
 * itself rides usbmux and never notices the LAN going away. The Settings
 * labels are matched in English, so the device has to be set to it.
 */
import { ASYNC_SCRIPT_TIMEOUT } from '../../helpers/timeouts';
import { switchToWebview } from '../webview';
import {
	WIFI_REASSOCIATE_MS,
	type WifiInfo,
	networkOf,
	waitForWifi,
} from '../wifi';
import { APP_BUNDLE_ID, APP_STATE_FOREGROUND, attachToIosApp } from './ios';

const SETTINGS_BUNDLE_ID = 'com.apple.Preferences';

/** How long a tapped "More Info" gets to push the network's info page before
 *  the tap is taken to have missed. */
const INFO_PAGE_MS = 5_000;

/** How many times a tap that shows no effect is repeated. */
const TAP_ATTEMPTS = 3;

/** How long a tap gets to show its effect before it is taken to have missed:
 *  a switch flipping, or a tapped network starting to associate. */
const TAP_TOOK_MS = 5_000;

/** What the root screen's Wi-Fi row names instead of an SSID. */
const NO_NETWORK_LABELS = ['Off', 'Not Connected'];

function classChain(chain: string): string {
	return `-ios class chain:${chain}`;
}

/** `text` quoted for a class chain predicate. */
function quoted(text: string): string {
	if (text.includes('"')) {
		throw new Error(`cannot match a Wi-Fi network named ${text} in Settings`);
	}
	return `"${text}"`;
}

/** The Settings app's Wi-Fi screens: the root row that names the current
 *  network, the Wi-Fi page with its switch and network list, and a network's
 *  info page. Each method says which screen it expects to be on. */
class SettingsApp {
	constructor(private readonly b: WebdriverIO.Browser) {}

	private rootWifiRow() {
		return this.b.$('~com.apple.settings.wifi');
	}

	private wifiSwitch() {
		// The label's hyphen is U+2011 on some releases and U+002D on others.
		return this.b.$(
			classChain('**/XCUIElementTypeSwitch[`label BEGINSWITH "Wi"`]'),
		);
	}

	private backButton() {
		return this.b.$(
			classChain('**/XCUIElementTypeNavigationBar/XCUIElementTypeButton[1]'),
		);
	}

	/** Root: the SSID the Wi-Fi row names, '' while Wi-Fi is off or on no
	 *  network. */
	async ssid(): Promise<string> {
		const label = (await this.rootWifiRow().getAttribute('label')) ?? '';
		const value = label.replace(/^[^,]*, /, '');
		return NO_NETWORK_LABELS.includes(value) ? '' : value;
	}

	/** Root -> Wi-Fi page. */
	async openWifi(): Promise<void> {
		await this.rootWifiRow().click();
		await this.wifiSwitch().waitForExist();
	}

	/** Wi-Fi page, or a page under it -> root. */
	async backToRoot(): Promise<void> {
		await this.backButton().click();
		if (!(await walkBackToRoot(this.b))) {
			throw new Error('Settings did not get back to its root screen');
		}
	}

	/** Wi-Fi page: flip the switch to `on` if it is not there already. A tap
	 *  now and then does not take, so it is repeated until the switch reads
	 *  as wanted. */
	async setWifi(on: boolean): Promise<void> {
		const wanted = on ? '1' : '0';
		const wifiSwitch = this.wifiSwitch();
		for (let attempt = 1; attempt <= TAP_ATTEMPTS; attempt++) {
			if ((await wifiSwitch.getAttribute('value')) === wanted) return;
			await wifiSwitch.click();
			const flipped = await this.b
				.waitUntil(
					async () => (await wifiSwitch.getAttribute('value')) === wanted,
					{ timeout: TAP_TOOK_MS },
				)
				.then(() => true)
				.catch(() => false);
			if (flipped) return;
		}
		throw new Error(`the Wi-Fi switch never turned ${on ? 'on' : 'off'}`);
	}

	private infoPageBar(ssid: string) {
		return this.b.$(
			classChain(
				`**/XCUIElementTypeNavigationBar[\`name == ${quoted(ssid)}\`]`,
			),
		);
	}

	/** The "More Info" button of a row named `ssid`, in one lookup: the list
	 *  can briefly hold a row of that name without the button, and a row
	 *  matched on its own would then be waited on for the whole timeout. */
	private moreInfoButton(ssid: string) {
		return this.b.$(
			classChain(
				`**/XCUIElementTypeCell[\`name BEGINSWITH ${quoted(`${ssid},`)}\`]/**/XCUIElementTypeButton[\`name == "More Info"\`]`,
			),
		);
	}

	/** Wi-Fi page -> `ssid`'s info page. The list re-renders as scans come in,
	 *  and a tap on a row replaced in between reports success without opening
	 *  anything, so the tap is repeated until the page is up. */
	async openInfo(ssid: string): Promise<void> {
		const bar = this.infoPageBar(ssid);
		await this.b.waitUntil(
			async () => {
				if (await bar.isExisting()) return true;
				const button = this.moreInfoButton(ssid);
				if (!(await button.isExisting())) return false;
				await button.click();
				return await bar
					.waitForExist({ timeout: INFO_PAGE_MS })
					.then(() => true)
					.catch(() => false);
			},
			{ timeoutMsg: `the info page of "${ssid}" never opened` },
		);
	}

	/** Info page: the IPv4 address, '' while the network has not handed one
	 *  out. The cell has no name of its own; "IP Address" is its label child. */
	async ipAddress(): Promise<string> {
		const value = this.b.$(
			classChain(
				'**/XCUIElementTypeCell[$name == "IP Address"$]/XCUIElementTypeStaticText[2]',
			),
		);
		if (!(await value.isExisting())) return '';
		return (await value.getAttribute('value')) ?? '';
	}
}

/** How long a freshly launched Settings gets to show its root screen. */
const SETTINGS_ROOT_MS = 5_000;

/** Tap back until the root screen shows, at most `pages` times; whether it
 *  did. Where Settings is when an operation ends is not always where it was
 *  expected: tapping the row of the network the phone has meanwhile joined
 *  on its own opens that network's page rather than joining, and a relaunch
 *  now and then comes up on the page it was last on. */
async function walkBackToRoot(
	b: WebdriverIO.Browser,
	pages = 4,
): Promise<boolean> {
	const root = b.$('~com.apple.settings.wifi');
	const back = b.$(
		classChain('**/XCUIElementTypeNavigationBar/XCUIElementTypeButton[1]'),
	);
	for (let page = 0; page <= pages; page++) {
		const atRoot = await root
			.waitForExist({ timeout: SETTINGS_ROOT_MS })
			.then(() => true)
			.catch(() => false);
		if (atRoot) return true;
		if (!(await back.isExisting())) return false;
		await back.click();
	}
	return false;
}

/** Whatever system alert is sitting over Settings — a network that could not
 *  be joined, a password that was wrong. Read only: the session's
 *  `autoAcceptAlerts` capability runs WDA's alert monitor, which taps the
 *  default button on these as they appear, so answering
 *  one here would be a second policy on the same alert with no say in which
 *  lands first. This exists to name what is on screen when Settings is stuck. */
async function readSystemAlert(b: WebdriverIO.Browser): Promise<string | null> {
	try {
		return await b.getAlertText();
	} catch {
		return null;
	}
}

/** What Settings is showing, for a failure that can be acted on rather than
 *  guessed at. */
async function describeScreen(b: WebdriverIO.Browser): Promise<string> {
	const alert = await readSystemAlert(b);
	if (alert !== null) return `the alert "${alert}"`;
	try {
		const bar = await b
			.$(classChain('**/XCUIElementTypeNavigationBar'))
			.getAttribute('name');
		return bar ? `"${bar}"` : 'no navigation bar';
	} catch {
		return 'nothing readable';
	}
}

/** Launch Settings on its root screen, relaunching if walking back does
 *  not get there. */
async function openSettingsAtRoot(b: WebdriverIO.Browser): Promise<void> {
	for (let launch = 1; launch <= 3; launch++) {
		await b.terminateApp(SETTINGS_BUNDLE_ID);
		await b.activateApp(SETTINGS_BUNDLE_ID);
		if (await walkBackToRoot(b)) return;
	}
	throw new Error(
		'Settings never showed its root screen; it was showing ' +
			(await describeScreen(b)),
	);
}

/** Run `body` against a freshly opened Settings app and put things back:
 *  Settings closed, the app on screen again if it was, the session in the
 *  app's webview if it was. A backgrounded or stopped app is left so. */
async function inSettings<T>(
	b: WebdriverIO.Browser,
	body: (settings: SettingsApp) => Promise<T>,
): Promise<T> {
	const context = await b.getContext();
	const contextId = typeof context === 'string' ? context : context.id;
	const wasOnScreen =
		Number(await b.queryAppState(APP_BUNDLE_ID)) === APP_STATE_FOREGROUND;
	await b.switchContext('NATIVE_APP');
	await openSettingsAtRoot(b);
	const settings = new SettingsApp(b);
	try {
		return await body(settings);
	} finally {
		await b.terminateApp(SETTINGS_BUNDLE_ID);
		if (wasOnScreen) {
			await b.activateApp(APP_BUNDLE_ID);
			if (contextId.startsWith('WEBVIEW')) await switchToWebview(b, 'ios');
		}
	}
}

/** Root -> the address `ssid` handed out, once it has. */
async function waitForAddressOn(
	settings: SettingsApp,
	ssid: string,
): Promise<string> {
	await settings.openWifi();
	await settings.openInfo(ssid);
	return await waitForWifi(
		() => settings.ipAddress(),
		`device never obtained a wifi address ${WIFI_REASSOCIATE_MS / 1_000}s after joining "${ssid}"`,
	);
}

/** Run `body` with the app on screen and the session in its webview, where
 *  `window.__test` lives — the fuzz restores networks between sequences
 *  with apps stopped or backgrounded, and a Settings operation leaves the
 *  session wherever it found it — and with an async-script timeout the
 *  plugin's calls fit in: an iOS session starts with none, and
 *  `executeAsync` gives up at once under it. */
async function inWebview<T>(
	b: WebdriverIO.Browser,
	body: () => Promise<T>,
): Promise<T> {
	const state = Number(
		await b.execute('mobile: queryAppState', { bundleId: APP_BUNDLE_ID }),
	);
	if (state !== APP_STATE_FOREGROUND) {
		await attachToIosApp(b);
	} else {
		const context = await b.getContext();
		const contextId = typeof context === 'string' ? context : context.id;
		if (!contextId.startsWith('WEBVIEW')) await switchToWebview(b, 'ios');
	}
	await b.setTimeout({ script: ASYNC_SCRIPT_TIMEOUT });
	return await body();
}

/** What a plugin call answers through `executeAsync`: its value, or its
 *  error, since a rejection would otherwise never call back and the script
 *  would time out with no word of why. One optional-field shape rather than
 *  a union, which the driver's typing of the callback cannot infer. */
type Reply<T> = { ok?: T; err?: string };

function unwrap<T>(reply: Reply<T>, what: string): T {
	if (reply.err !== undefined) throw new Error(`${what}: ${reply.err}`);
	return reply.ok as T;
}

/** The interface as the app reads it, for a session already in the webview:
 *  the address for free, the SSID only for a network the app added itself —
 *  a lab network — since iOS hides every other SSID from an app. */
async function readWifiInfo(b: WebdriverIO.Browser): Promise<WifiInfo> {
	const { ssid, address, prefixLength } = unwrap(
		await b.executeAsync(
			(
				done: (
					r: Reply<{ ssid: string; address: string; prefixLength: number }>,
				) => void,
			) => {
				window.__test.wifi.current().then(
					ok => done({ ok }),
					e => done({ err: String(e) }),
				);
			},
		),
		'reading the Wi-Fi interface',
	);
	return { ssid, address, network: networkOf(address, prefixLength) };
}

async function readAddedSsids(b: WebdriverIO.Browser): Promise<string[]> {
	return unwrap(
		await b.executeAsync((done: (r: Reply<string[]>) => void) => {
			window.__test.wifi.addedSsids().then(
				r => done({ ok: r.ssids }),
				e => done({ err: String(e) }),
			);
		}),
		'listing the added networks',
	);
}

async function forgetSsid(b: WebdriverIO.Browser, ssid: string): Promise<void> {
	unwrap(
		await b.executeAsync((ssid: string, done: (r: Reply<null>) => void) => {
			window.__test.wifi.forget(ssid).then(
				() => done({ ok: null }),
				e => done({ err: String(e) }),
			);
		}, ssid),
		`forgetting "${ssid}"`,
	);
}

/** Drop every lab network the app added but `keep`. */
async function forgetAdded(b: WebdriverIO.Browser, keep = ''): Promise<void> {
	for (const ssid of await readAddedSsids(b)) {
		if (ssid !== keep) await forgetSsid(b, ssid);
	}
}

/** The network the device is on; see [`readWifiInfo`]. */
export function iosWifiInfo(b: WebdriverIO.Browser): Promise<WifiInfo> {
	return inWebview(b, () => readWifiInfo(b));
}

/** The lab networks the app has added, in range or not. */
export function iosAddedSsids(b: WebdriverIO.Browser): Promise<string[]> {
	return inWebview(b, () => readAddedSsids(b));
}

/** Drop every lab network the app added; the device leaves it if it is on
 *  one. Resolves at once, without waiting for where it lands. */
export function forgetIosWifi(b: WebdriverIO.Browser): Promise<void> {
	return inWebview(b, () => forgetAdded(b));
}

/** Turn the radio on without waiting for where it lands: what a join asks
 *  of a device that holds no address, since the join itself decides where
 *  it goes. A no-op when the radio is on already. */
function radioOn(b: WebdriverIO.Browser): Promise<void> {
	return inSettings(b, async settings => {
		await settings.openWifi();
		await settings.setWifi(true);
	});
}

/** How often the join request is looked in on. */
const JOIN_POLL_MS = 500;

/** Answer the "join this network?" alert iOS puts up for a join request as
 *  it comes, and resolve with the request's outcome once it has one. Both
 *  are watched together: the alert may come late on a busy device, the
 *  session's own alert monitor may take it first, and while it is up every
 *  webview command is refused as blocked by a modal. */
async function settleJoinRequest(
	b: WebdriverIO.Browser,
	ssid: string,
): Promise<string> {
	const deadline = Date.now() + WIFI_REASSOCIATE_MS;
	while (Date.now() < deadline) {
		await b.switchContext('NATIVE_APP');
		if (await b.$(classChain('**/XCUIElementTypeAlert')).isExisting()) {
			try {
				await b.acceptAlert();
			} catch {
				/* taken by the alert monitor in between */
			}
		}
		await switchToWebview(b, 'ios');
		try {
			const outcome = await b.execute(
				(ssid: string) => window.__test.wifi.requestJoinOutcome(ssid),
				ssid,
			);
			if (outcome !== null) return outcome;
		} catch {
			/* the alert came up in between; the next round answers it */
		}
		await b.pause(JOIN_POLL_MS);
	}
	throw new Error(
		`joining "${ssid}" was still going ${WIFI_REASSOCIATE_MS / 1_000}s later`,
	);
}

/** Join `ssid` (an empty `passphrase` means an open network) as the app's
 *  one lab network — any other the app added is dropped once the device is
 *  on this one — and resolve with the IPv4 address obtained on it. A device
 *  already associated with it is only waited for: iOS refuses a request for
 *  the network it is on. The request is started and left running, its alert
 *  answered natively, and its outcome read back: awaiting it from the
 *  webview would have the driver retry the blocked call and iOS refuse the
 *  second request. */
export function joinIosWifi(
	b: WebdriverIO.Browser,
	ssid: string,
	passphrase: string,
): Promise<string> {
	return inWebview(b, async () => {
		const before = await readWifiInfo(b);
		if (before.ssid === ssid) {
			await forgetAdded(b, ssid);
			return await waitForWifi(
				async () => (await readWifiInfo(b)).address,
				`device never obtained an address ${WIFI_REASSOCIATE_MS / 1_000}s after associating with "${ssid}"`,
			);
		}
		if (before.address === '') await radioOn(b);
		await b.execute(
			(ssid: string, passphrase: string) =>
				window.__test.wifi.startRequestJoin(ssid, passphrase),
			ssid,
			passphrase,
		);
		const outcome = await settleJoinRequest(b, ssid);
		if (outcome !== 'ok') throw new Error(`joining "${ssid}": ${outcome}`);
		await forgetAdded(b, ssid);
		return (await readWifiInfo(b)).address;
	});
}

/** Drop every lab network the app added and resolve with where the device
 *  is once it has settled on a network of its own — whichever the user
 *  saved; the harness never learns its name. A device on a lab network is
 *  waited for until it holds an address on another LAN: the SSID reads ''
 *  the moment the configuration is gone, before the association drops, and
 *  a lab network serves a subnet of its own (see `.env.example`). One on no
 *  lab network keeps what it has, and one off the air gets its radio turned
 *  on. */
export function leaveIosWifi(b: WebdriverIO.Browser): Promise<WifiInfo> {
	return inWebview(b, async () => {
		const before = await readWifiInfo(b);
		await forgetAdded(b);
		if (before.ssid === '') {
			if (before.address === '') await enableIosWifi(b);
			return await readWifiInfo(b);
		}
		await waitForWifi(
			async () => {
				const { ssid, network } = await readWifiInfo(b);
				return ssid === '' && network !== '' && network !== before.network
					? network
					: '';
			},
			`device never settled on a network of its own ${WIFI_REASSOCIATE_MS / 1_000}s after leaving "${before.ssid}"`,
		);
		return await readWifiInfo(b);
	});
}

export function disableIosWifi(b: WebdriverIO.Browser): Promise<void> {
	return inSettings(b, async settings => {
		await settings.openWifi();
		await settings.setWifi(false);
	});
}

/** Turn Wi-Fi on and resolve with the IPv4 address the device came back on. */
export function enableIosWifi(b: WebdriverIO.Browser): Promise<string> {
	return inSettings(b, async settings => {
		await settings.openWifi();
		await settings.setWifi(true);
		await settings.backToRoot();
		const ssid = await waitForWifi(
			() => settings.ssid(),
			`device never rejoined a network ${WIFI_REASSOCIATE_MS / 1_000}s after re-enabling`,
		);
		return await waitForAddressOn(settings, ssid);
	});
}
