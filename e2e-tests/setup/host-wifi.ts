/**
 * The host's Wi-Fi card as a client of the networks a run may use: joining
 * one is what puts the hubs on that LAN, since every hub is a process on
 * the host. NetworkManager does the joining; the host keeps its wired link
 * for everything else, and the card autoconnects back to its usual network
 * once the run lets it go.
 */
import { execFileSync } from 'node:child_process';

function nmcli(...args: string[]): string {
	return execFileSync('nmcli', args, { encoding: 'utf8' });
}

/** The host's first Wi-Fi device, or null when it has none. */
export function wifiDevice(): string | null {
	const found = nmcli('-t', '-f', 'DEVICE,TYPE', 'device')
		.split('\n')
		.map(line => line.split(':'))
		.find(([, type]) => type === 'wifi');
	return found === undefined ? null : found[0];
}

/** Join `ssid` on `device` and resolve with the IPv4 address obtained on it. */
export async function joinWifi(
	device: string,
	ssid: string,
	passphrase: string,
): Promise<string> {
	// NetworkManager refuses while the card is still re-joining its usual
	// network after the previous leave.
	const joinBy = Date.now() + 60_000;
	for (;;) {
		try {
			// `connect` only trusts the scan cache, which goes stale while the
			// card sits on its usual network; this scan blocks until it is fresh.
			nmcli('device', 'wifi', 'list', 'ifname', device, '--rescan', 'yes');
			nmcli(
				'device',
				'wifi',
				'connect',
				ssid,
				...(passphrase === '' ? [] : ['password', passphrase]),
				'ifname',
				device,
			);
			break;
		} catch (err) {
			if (Date.now() > joinBy) {
				throw new Error(
					`could not join "${ssid}" on ${device}: ${String(err)}`,
				);
			}
			leaveWifi(ssid);
			await new Promise(resolve => setTimeout(resolve, 3_000));
		}
	}
	const deadline = Date.now() + 20_000;
	for (;;) {
		const address = nmcli('-g', 'IP4.ADDRESS', 'device', 'show', device)
			.trim()
			.split('/')[0];
		if (address !== '') {
			console.log(`[wifi] ${device} joined ${ssid} at ${address}`);
			return address;
		}
		if (Date.now() > deadline) {
			leaveWifi(ssid);
			throw new Error(`joined "${ssid}" but ${device} never got an address`);
		}
		await new Promise(resolve => setTimeout(resolve, 500));
	}
}

/** Leave `ssid` and drop the profile the join created; the card reconnects
 *  to its usual network on its own. */
export function leaveWifi(ssid: string): void {
	for (const action of ['down', 'delete']) {
		try {
			execFileSync('nmcli', ['connection', action, ssid], {
				stdio: 'ignore',
			});
		} catch {
			/* not joined, or already gone */
		}
	}
}
