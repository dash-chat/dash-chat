/** NetworkManager does the joining; the card autoconnects back to its usual
 *  network once a lab network's profile is dropped. */
import { execFileSync } from 'node:child_process';

import { type HostWifi, waitForAddress } from './driver';

function nmcli(...args: string[]): string {
	return execFileSync('nmcli', args, { encoding: 'utf8' });
}

/** Drops the profile the join created as well, so the card cannot pick
 *  the network again. */
function leaveWifi(ssid: string): void {
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

export const linux: HostWifi = {
	wifiDevice() {
		const found = nmcli('-t', '-f', 'DEVICE,TYPE', 'device')
			.split('\n')
			.map(line => line.split(':'))
			.find(([, type]) => type === 'wifi');
		return found === undefined ? null : found[0];
	},

	visibleNetworks(device) {
		const ssids = nmcli(
			'-t',
			'-f',
			'SSID',
			'device',
			'wifi',
			'list',
			'ifname',
			device,
			'--rescan',
			'yes',
		)
			.split('\n')
			.filter(ssid => ssid !== '');
		return [...new Set(ssids)];
	},

	async joinWifi(device, ssid, passphrase) {
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
		const address = await waitForAddress(
			() =>
				nmcli('-g', 'IP4.ADDRESS', 'device', 'show', device)
					.trim()
					.split('/')[0],
			() => leaveWifi(ssid),
			`joined "${ssid}" but ${device} never got an address`,
		);
		console.log(`[wifi] ${device} joined ${ssid} at ${address}`);
		return address;
	},

	leaveWifi,
};
