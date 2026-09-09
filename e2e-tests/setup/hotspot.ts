/**
 * Wi-Fi networks the host raises for a spec, one per Wi-Fi card: the LANs a
 * hub can live on, separate from the one the phones normally use.
 * NetworkManager turns a card into an access point — the host keeps its wired
 * link for everything else — and hands out addresses in a subnet of its own.
 * The credentials are made up here, so nothing has to be configured. Bringing
 * a network down puts its card back on whatever it was connected to before.
 */
import { execFileSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';

export interface Hotspot {
	/** The SSID, also the NetworkManager connection name. */
	ssid: string;
	passphrase: string;
	/** The Wi-Fi device it runs on. */
	device: string;
	/** The host's IPv4 address on the hotspot's LAN. */
	address: string;
}

function nmcli(...args: string[]): string {
	return execFileSync('nmcli', args, { encoding: 'utf8' });
}

/** The host's Wi-Fi devices: one hotspot can run on each. */
export function wifiDevices(): string[] {
	return nmcli('-t', '-f', 'DEVICE,TYPE', 'device')
		.split('\n')
		.map(line => line.split(':'))
		.filter(([, type]) => type === 'wifi')
		.map(([device]) => device);
}

/** Raise a hotspot called `ssid` on `device` and resolve once the host holds
 *  an address on it. */
export async function startHotspot(
	ssid: string,
	device: string,
): Promise<Hotspot> {
	// A previous run that died mid-way leaves the profile behind.
	stopHotspot(ssid);
	const passphrase = randomBytes(6).toString('hex');
	// NetworkManager refuses ("activation queued") while the card is still
	// re-joining its usual network after a previous hotspot went down.
	const activateBy = Date.now() + 60_000;
	for (;;) {
		try {
			nmcli(
				'device',
				'wifi',
				'hotspot',
				'ifname',
				device,
				'con-name',
				ssid,
				'ssid',
				ssid,
				'password',
				passphrase,
				'band',
				'bg',
			);
			break;
		} catch (err) {
			if (Date.now() > activateBy) throw err;
			stopHotspot(ssid);
			await new Promise(resolve => setTimeout(resolve, 3_000));
		}
	}
	const deadline = Date.now() + 20_000;
	for (;;) {
		const address = nmcli('-g', 'IP4.ADDRESS', 'device', 'show', device)
			.trim()
			.split('/')[0];
		if (address !== '') {
			console.log(`[hotspot] ${ssid} up on ${device} at ${address}`);
			return { ssid, passphrase, device, address };
		}
		if (Date.now() > deadline) {
			stopHotspot(ssid);
			throw new Error(`${ssid} came up but ${device} never got an address`);
		}
		await new Promise(resolve => setTimeout(resolve, 500));
	}
}

/** Drop the hotspot called `ssid`; its card reconnects to its usual network
 *  on its own. */
export function stopHotspot(ssid: string): void {
	for (const action of ['down', 'delete']) {
		try {
			execFileSync('nmcli', ['connection', action, ssid], {
				stdio: 'ignore',
			});
		} catch {
			/* not up, or already gone */
		}
	}
}
