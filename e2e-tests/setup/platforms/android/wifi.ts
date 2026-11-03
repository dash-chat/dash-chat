/**
 * Wi-Fi control for a physical Android phone over adb. Every operation takes
 * the phone from whatever state it is in to the one it names. A phone's
 * Wi-Fi service decides how: most let adb join a network outright
 * (`cmd wifi connect-network`), and some vendor builds (Transsion's HiOS)
 * withhold that and leave adb only network suggestions, which a phone joins
 * on its own once no network it knows better is in range.
 */
import { hostNetwork } from '../../host-wifi';
import type { WifiNetwork } from '../../test-env';
import {
	type PhoneWifi,
	WIFI_REASSOCIATE_MS,
	type WifiInfo,
	hostNetworkVerdict,
	networkOf,
	waitForWifi,
} from '../../wifi';
import { adbShell } from './index';

export function androidWifi(udid: string, testNetworks: string[]): PhoneWifi {
	return joinsOutright(udid)
		? connectingWifi(udid, testNetworks)
		: suggestingWifi(udid, testNetworks);
}

const joinsOutrightByUdid = new Map<string, boolean>();

/** Whether adb may join networks on `udid` outright: a Wi-Fi service lists
 *  to the shell only the commands it lets the shell run, and some exit
 *  non-zero after printing that list. No list at all is a Wi-Fi service not
 *  up yet, which says nothing and is asked again next time. */
function joinsOutright(udid: string): boolean {
	let known = joinsOutrightByUdid.get(udid);
	if (known === undefined) {
		const help = adbShell(udid, '"cmd wifi help 2>&1 || true"');
		if (!/^\s+help\b/m.test(help)) {
			throw new Error(
				`device ${udid}: its Wi-Fi service did not list its commands: ${help.trim()}`,
			);
		}
		known = /^\s+connect-network /m.test(help);
		joinsOutrightByUdid.set(udid, known);
	}
	return known;
}

/** A phone adb joins networks on outright. Its own saved networks are left
 *  alone until it is surely on the host's LAN; home is joined by name when
 *  the host's network is known, and fallen back to otherwise, or when the
 *  join by name does not get the phone there. */
function connectingWifi(udid: string, testNetworks: string[]): PhoneWifi {
	return {
		async join({ ssid, passphrase }) {
			const address = await connectAndroidWifi(udid, ssid, passphrase);
			forgetSavedWifi(
				udid,
				other => other !== ssid && testNetworks.includes(other),
			);
			return address;
		},
		async goHome() {
			const home = hostNetwork(testNetworks);
			if (home === null) return await leaveAndroidWifi(udid, testNetworks);
			forgetSavedWifi(udid, ssid => testNetworks.includes(ssid));
			try {
				await connectAndroidWifi(udid, home.ssid, home.passphrase);
			} catch (err) {
				console.warn(
					`[wifi] ${udid}: joining the host's "${home.ssid}" by name failed, ` +
						`falling back to its saved networks: ${String(err)}`,
				);
				return await leaveAndroidWifi(udid, testNetworks);
			}
			const info = androidWifiInfo(udid);
			// Forgetting cannot be undone, so it waits for the host to vouch for
			// the network, which it cannot while its card is on a test network.
			if (hostNetworkVerdict(info.address) === 'host') {
				forgetSavedWifi(udid, ssid => ssid !== home.ssid);
			}
			return info;
		},
		dropTestNetworks: async () =>
			forgetSavedWifi(udid, ssid => testNetworks.includes(ssid)),
		radioOff: () => disableAndroidWifi(udid),
		radioOn: () => enableAndroidWifi(udid),
	};
}

/** A phone adb can only suggest networks to. A saved network in range
 *  outranks every suggestion, and the phone stays on a suggestion it is on
 *  while that is still suggested, so the phone is kept with no saved
 *  network and exactly one suggestion: the network it is to be on. The
 *  host's network is one of them, which is why it has to be known. */
function suggestingWifi(udid: string, testNetworks: string[]): PhoneWifi {
	return {
		join: network => suggestOnly(udid, network),
		async goHome() {
			const home = hostNetwork(testNetworks);
			if (home !== null) {
				await suggestOnly(udid, home);
				return androidWifiInfo(udid);
			}
			const here = androidWifiInfo(udid);
			if (hostNetworkVerdict(here.address) === 'host') return here;
			throw new Error(
				`device ${udid} only takes Wi-Fi suggestions from adb, so it can ` +
					"only be put back on the host's network by name, and this " +
					'host does not tell it: set E2E_HOST_WIFI=ssid:passphrase in ' +
					'e2e-tests/.env',
			);
		},
		async dropTestNetworks() {
			const home = hostNetwork(testNetworks);
			if (home !== null) suggestAndroidWifi(udid, home);
			removeSuggestions(udid, ssid => testNetworks.includes(ssid));
		},
		radioOff: () => disableAndroidWifi(udid),
		radioOn: () => enableAndroidWifi(udid),
	};
}

/** Put `udid` on `network` as the one network it may land on, and resolve
 *  with the address it obtained there. Its saved networks are only
 *  forgotten once the suggestion is known to stand a chance. */
async function suggestOnly(
	udid: string,
	network: WifiNetwork,
): Promise<string> {
	adbShell(udid, 'svc wifi enable');
	suggestAndroidWifi(udid, network);
	if (!shellSuggestionsApproved(udid)) {
		throw new Error(
			`device ${udid} has not approved the Wi-Fi suggestions adb makes: ` +
				'accept the "suggested networks" notification for Shell on it',
		);
	}
	const saved = new Set(savedAndroidWifi(udid).map(({ ssid }) => ssid));
	if (saved.size > 0) {
		console.log(
			`[wifi] ${udid}: forgetting ${[...saved].map(s => `"${s}"`).join(', ')}, ` +
				'saved networks outranking the suggestions it is driven by',
		);
		forgetSavedWifi(udid, () => true);
	}
	removeSuggestions(udid, ssid => ssid !== network.ssid);
	return await associated(udid, network.ssid);
}

function suggestAndroidWifi(
	udid: string,
	{ ssid, passphrase }: WifiNetwork,
): void {
	adbShellWithSecret(
		udid,
		`cmd wifi add-suggestion ${deviceShellQuote(ssid)} ${cmdWifiSecurity(passphrase)}`,
		passphrase,
	);
}

/** Withdraw every suggestion adb has made whose SSID `remove` picks. */
function removeSuggestions(
	udid: string,
	remove: (ssid: string) => boolean,
): void {
	for (const ssid of suggestedAndroidWifi(udid)) {
		if (remove(ssid)) {
			adbShell(udid, `cmd wifi remove-suggestion ${deviceShellQuote(ssid)}`);
		}
	}
}

/** The SSIDs adb has suggested, below a header row, one per row with its
 *  security types last. */
function suggestedAndroidWifi(udid: string): string[] {
	return adbShell(udid, 'cmd wifi list-suggestions')
		.split('\n')
		.slice(1)
		.map(line => line.match(/^(.+?)\s+\S+\s*$/)?.[1])
		.filter(ssid => ssid !== undefined);
}

/** Filtered on the device: a whole `dumpsys wifi` can outgrow the host's
 *  output buffer. */
function shellSuggestionsApproved(udid: string): boolean {
	return /Has user approved: true/.test(
		adbShell(
			udid,
			'"dumpsys wifi | grep -A1 \'Package Name: com.android.shell\' || true"',
		),
	);
}

/** Run `command`, which carries `secret`, keeping the secret out of the
 *  error it may throw: a failed command is quoted whole in its message. */
function adbShellWithSecret(
	udid: string,
	command: string,
	secret: string,
): string {
	try {
		return adbShell(udid, command);
	} catch (err) {
		if (secret === '') throw err;
		const message = String(err)
			.split(deviceShellQuote(secret))
			.join('<secret>')
			.split(secret)
			.join('<secret>');
		throw new Error(message);
	}
}

export function androidWifiInfo(udid: string): WifiInfo {
	const [address, prefixLength] = androidWifiInterface(udid);
	return {
		ssid: androidWifiSsid(udid),
		address,
		network: networkOf(address, prefixLength),
	};
}

/** The IPv4 address and prefix length of wlan0; '' and 0 while it has none.
 *  While wifi is down the interface itself disappears and adb exits
 *  non-zero, which is the same "no address yet" answer as an empty match. */
function androidWifiInterface(udid: string): [string, number] {
	try {
		const out = adbShell(udid, 'ip -4 addr show wlan0');
		const match = out.match(/inet (\d+\.\d+\.\d+\.\d+)\/(\d+)/);
		return match === null ? ['', 0] : [match[1], Number(match[2])];
	} catch {
		return ['', 0];
	}
}

function androidWifiAddress(udid: string): string {
	return androidWifiInterface(udid)[0];
}

/** Quote `value` for the device's shell, through the host shell that `adb
 *  shell` hands it to first: the host strips the double quotes, the device the
 *  single ones, and the text arrives verbatim. */
function deviceShellQuote(value: string): string {
	const single = `'${value.replace(/'/g, "'\\''")}'`;
	return `"${single.replace(/(["$`\\])/g, '\\$1')}"`;
}

function cmdWifiSecurity(passphrase: string): string {
	return passphrase === '' ? 'open' : `wpa2 ${deviceShellQuote(passphrase)}`;
}

/** The SSID the device is associated with, or '' while it is on none. */
function androidWifiSsid(udid: string): string {
	try {
		return (
			adbShell(udid, 'cmd wifi status').match(
				/Wifi is connected to "(.*)"/,
			)?.[1] ?? ''
		);
	} catch {
		return '';
	}
}

/** Join `ssid` (an empty `passphrase` means an open network), saving it on
 *  the device if it is new, and resolve with the IPv4 address obtained on it.
 *  A device already on it is not made to reassociate. */
async function connectAndroidWifi(
	udid: string,
	ssid: string,
	passphrase: string,
): Promise<string> {
	adbShell(udid, 'svc wifi enable');
	if (androidWifiSsid(udid) !== ssid) {
		adbShellWithSecret(
			udid,
			`cmd wifi connect-network ${deviceShellQuote(ssid)} ${cmdWifiSecurity(passphrase)}`,
			passphrase,
		);
	}
	return await associated(udid, ssid);
}

/** Resolve with the IPv4 address `udid` holds once it is on `ssid`. */
async function associated(udid: string, ssid: string): Promise<string> {
	await waitForWifi(
		() => (androidWifiSsid(udid) === ssid ? ssid : ''),
		`device never associated with "${ssid}" within ${WIFI_REASSOCIATE_MS / 1_000}s; is it in range, and are the credentials right?`,
	);
	return await waitForWifi(
		() => androidWifiAddress(udid),
		`device never obtained a wifi address ${WIFI_REASSOCIATE_MS / 1_000}s after joining "${ssid}"`,
	);
}

/** The networks saved on `udid`. A network is listed once per security type
 *  it accepts, so its id can come up twice. */
function savedAndroidWifi(udid: string): { id: string; ssid: string }[] {
	return adbShell(udid, 'cmd wifi list-networks')
		.split('\n')
		.map(line => line.match(/^(\d+)\s+(.+?)\s+\S+\s*$/))
		.filter(match => match !== null)
		.map(([, id, ssid]) => ({ id, ssid }));
}

/** Forget every saved network whose SSID `forget` picks, without waiting
 *  for where the device lands. */
function forgetSavedWifi(
	udid: string,
	forget: (ssid: string) => boolean,
): void {
	const ids = savedAndroidWifi(udid)
		.filter(({ ssid }) => forget(ssid))
		.map(({ id }) => id);
	for (const id of new Set(ids)) {
		adbShell(udid, `cmd wifi forget-network ${id}`);
	}
}

/** Get the device onto the network the host is on from wherever it is: radio
 *  on, every one of `testNetworks` forgotten, and then every other network it
 *  lands on that is clearly not the host's forgotten in turn. Once it is
 *  surely on the host's network every other saved network goes too, so all it
 *  can ever fall back to is that one; while that is unsure nothing more is
 *  forgotten, and the caller's reachability check names the network. */
async function leaveAndroidWifi(
	udid: string,
	testNetworks: string[],
): Promise<WifiInfo> {
	adbShell(udid, 'svc wifi enable');
	forgetSavedWifi(udid, ssid => testNetworks.includes(ssid));
	let left: WifiInfo | null = null;
	for (;;) {
		const info = await settledAndroidWifi(udid, left);
		const verdict = hostNetworkVerdict(info.address);
		if (verdict === 'host') forgetSavedWifi(udid, ssid => ssid !== info.ssid);
		if (verdict !== 'elsewhere') return info;
		forgetSavedWifi(udid, ssid => ssid === info.ssid);
		left = info;
	}
}

/** Where the device is once it is associated and holds an address, on a
 *  network other than `left`: the SSID changes before the interface drops
 *  the old address, so both have to have moved on. */
async function settledAndroidWifi(
	udid: string,
	left: WifiInfo | null,
): Promise<WifiInfo> {
	await waitForWifi(
		() => {
			const { ssid, address } = androidWifiInfo(udid);
			if (ssid === '' || address === '') return '';
			if (left !== null && (ssid === left.ssid || address === left.address)) {
				return '';
			}
			return ssid;
		},
		`device settled on no network the host can reach within ${WIFI_REASSOCIATE_MS / 1_000}s; ` +
			"save the host's Wi-Fi on it once by hand, or set E2E_HOST_WIFI",
	);
	return androidWifiInfo(udid);
}

/** Turn Wi-Fi off and resolve once the device holds no Wi-Fi address: the
 *  radio takes a moment to drop after the command returns, and a peer that
 *  sends in that moment still gets through. */
async function disableAndroidWifi(udid: string): Promise<void> {
	adbShell(udid, 'svc wifi disable');
	await waitForWifi(
		() => (androidWifiAddress(udid) === '' ? 'off' : ''),
		`device still held a wifi address ${WIFI_REASSOCIATE_MS / 1_000}s after disabling`,
	);
}

/** Turn Wi-Fi on and resolve with the IPv4 address the device came back on. */
function enableAndroidWifi(udid: string): Promise<string> {
	adbShell(udid, 'svc wifi enable');
	return waitForWifi(
		() => androidWifiAddress(udid),
		`device never regained a wifi address ${WIFI_REASSOCIATE_MS / 1_000}s after re-enabling`,
	);
}
