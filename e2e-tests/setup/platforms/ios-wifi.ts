/**
 * Wi-Fi control for a physical iPhone, driven through the Settings app over
 * the XCUITest session: iOS has no shell to script the supplicant from, so
 * the harness taps what a user would. Every operation takes the app off
 * screen for its duration and puts it back — as a user changing networks
 * does — while the session itself rides usbmux and never notices the LAN
 * going away. The Settings labels are matched in English, so the device has
 * to be set to it.
 */
import { switchToWebview } from '../webview';
import { WIFI_REASSOCIATE_MS, type WifiInfo, waitForWifi } from '../wifi';
import { APP_BUNDLE_ID, APP_STATE_FOREGROUND } from './ios';

const SETTINGS_BUNDLE_ID = 'com.apple.Preferences';

/** How long a saved network gets to join without asking for its password
 *  before the join is taken to be waiting on one. */
const PASSWORD_SHEET_MS = 5_000;

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

	private networkRow(ssid: string) {
		return this.b.$(
			classChain(
				`**/XCUIElementTypeCell[\`name BEGINSWITH ${quoted(`${ssid},`)}\`]`,
			),
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

	/** Wi-Fi page -> root. */
	async backToRoot(): Promise<void> {
		await this.backButton().click();
		await this.rootWifiRow().waitForExist();
	}

	/** Wi-Fi page: flip the switch to `on` if it is not there already. */
	async setWifi(on: boolean): Promise<void> {
		const wanted = on ? '1' : '0';
		const wifiSwitch = this.wifiSwitch();
		if ((await wifiSwitch.getAttribute('value')) === wanted) return;
		await wifiSwitch.click();
		await this.b.waitUntil(
			async () => (await wifiSwitch.getAttribute('value')) === wanted,
			{ timeoutMsg: `the Wi-Fi switch never turned ${on ? 'on' : 'off'}` },
		);
	}

	/** Wi-Fi page: tap `ssid` in the list once the scan shows it, and answer
	 *  the password sheet if one comes up. */
	async join(ssid: string, passphrase: string): Promise<void> {
		const row = this.networkRow(ssid);
		await row.waitForExist({
			timeout: WIFI_REASSOCIATE_MS,
			timeoutMsg: `"${ssid}" never appeared in the Wi-Fi list within ${WIFI_REASSOCIATE_MS / 1_000}s; is it in range?`,
		});
		await row.click();
		const password = this.b.$(
			classChain('**/XCUIElementTypeSecureTextField[`name == "Password"`]'),
		);
		try {
			await password.waitForExist({ timeout: PASSWORD_SHEET_MS });
		} catch {
			return;
		}
		await password.click();
		await password.setValue(passphrase);
		// The sheet's own bar is the last one on screen; Join is its last button.
		const join = this.b.$(
			classChain(
				'**/XCUIElementTypeNavigationBar[-1]/XCUIElementTypeButton[-1]',
			),
		);
		await join.waitForEnabled();
		await join.click();
		// The sheet stays up over the page's own bar while iOS tries the
		// network, and goes away once it has joined; a wrong password leaves it
		// up, behind an alert the session auto-accepts.
		await password.waitForExist({
			reverse: true,
			timeout: WIFI_REASSOCIATE_MS,
			timeoutMsg: `"${ssid}" was still asking for its password ${WIFI_REASSOCIATE_MS / 1_000}s after it was entered; are the credentials right?`,
		});
	}

	/** Wi-Fi page -> `ssid`'s info page. */
	async openInfo(ssid: string): Promise<void> {
		await this.networkRow(ssid).$('~More Info').click();
		await this.b
			.$(
				classChain(
					`**/XCUIElementTypeNavigationBar[\`name == ${quoted(ssid)}\`]`,
				),
			)
			.waitForExist();
	}

	/** Info page: the IPv4 address, '' while the network has not handed one
	 *  out. */
	async ipAddress(): Promise<string> {
		const value = this.b.$(
			classChain(
				'**/XCUIElementTypeCell[`name == "IP Address"`]/XCUIElementTypeStaticText[2]',
			),
		);
		if (!(await value.isExisting())) return '';
		return (await value.getAttribute('value')) ?? '';
	}

	/** Info page -> Wi-Fi page, forgetting the network if it was saved. */
	async forget(): Promise<void> {
		const forget = this.b.$(
			classChain(
				'**/XCUIElementTypeStaticText[`name == "Forget This Network"`]',
			),
		);
		if (await forget.isExisting()) {
			await forget.click();
			const confirm = this.b.$(
				classChain('**/XCUIElementTypeButton[`name == "Forget"`]'),
			);
			// The session auto-accepts alerts, which may already have answered it.
			try {
				await confirm.waitForExist({ timeout: PASSWORD_SHEET_MS });
				await confirm.click();
			} catch {
				/* accepted for us */
			}
		} else {
			await this.backButton().click();
		}
		await this.wifiSwitch().waitForExist();
	}
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
	await b.terminateApp(SETTINGS_BUNDLE_ID);
	await b.activateApp(SETTINGS_BUNDLE_ID);
	const settings = new SettingsApp(b);
	await b.$('~com.apple.settings.wifi').waitForExist();
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

export function iosWifiInfo(b: WebdriverIO.Browser): Promise<WifiInfo> {
	return inSettings(b, async settings => {
		const ssid = await settings.ssid();
		if (ssid === '') return { ssid, address: '' };
		await settings.openWifi();
		await settings.openInfo(ssid);
		return { ssid, address: await settings.ipAddress() };
	});
}

/** Join `ssid` (an empty `passphrase` means an open network), saving it on
 *  the device if it is new, and resolve with the IPv4 address obtained on it. */
export function connectIosWifi(
	b: WebdriverIO.Browser,
	ssid: string,
	passphrase: string,
): Promise<string> {
	return inSettings(b, async settings => {
		if ((await settings.ssid()) !== ssid) {
			await settings.openWifi();
			await settings.setWifi(true);
			await settings.join(ssid, passphrase);
			await settings.backToRoot();
			await waitForWifi(
				async () => ((await settings.ssid()) === ssid ? ssid : ''),
				`device never associated with "${ssid}" within ${WIFI_REASSOCIATE_MS / 1_000}s; is it in range, and are the credentials right?`,
			);
		}
		return await waitForAddressOn(settings, ssid);
	});
}

/** Forget `ssid`, which drops the association if that is the current one,
 *  and resolve with the IPv4 address the device is on once it has settled on
 *  another saved network. */
export function forgetIosWifi(
	b: WebdriverIO.Browser,
	ssid: string,
): Promise<string> {
	return inSettings(b, async settings => {
		await settings.openWifi();
		await settings.openInfo(ssid);
		await settings.forget();
		await settings.backToRoot();
		const other = await waitForWifi(
			async () => {
				const current = await settings.ssid();
				return current === ssid ? '' : current;
			},
			`device never settled on another network ${WIFI_REASSOCIATE_MS / 1_000}s after forgetting "${ssid}"`,
		);
		return await waitForAddressOn(settings, other);
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
