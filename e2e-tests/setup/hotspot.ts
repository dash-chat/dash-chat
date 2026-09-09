/**
 * A Wi-Fi network the host raises for a spec: the LAN a hub lives on, separate
 * from the one the phones normally use. NetworkManager turns the host's Wi-Fi
 * card into an access point — the host keeps its wired link for everything
 * else — and hands out addresses in a subnet of its own. The credentials are
 * made up here, so nothing has to be configured. Bringing it down puts the
 * card back on whatever it was connected to before.
 */
import { execFileSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';

const CONNECTION_NAME = 'dash-e2e-hotspot';

export interface Hotspot {
	ssid: string;
	passphrase: string;
	/** The host's IPv4 address on the hotspot's LAN. */
	address: string;
}

function nmcli(...args: string[]): string {
	return execFileSync('nmcli', args, { encoding: 'utf8' });
}

function wifiDevice(): string {
	const device = nmcli('-t', '-f', 'DEVICE,TYPE', 'device')
		.split('\n')
		.map(line => line.split(':'))
		.find(([, type]) => type === 'wifi')?.[0];
	if (device === undefined) {
		throw new Error('the host has no Wi-Fi device to raise a hotspot on');
	}
	return device;
}

/** Raise the hotspot and resolve once the host holds an address on it. */
export async function startHotspot(): Promise<Hotspot> {
	// A previous run that died mid-way leaves the profile behind.
	stopHotspot();
	const device = wifiDevice();
	const ssid = CONNECTION_NAME;
	const passphrase = randomBytes(6).toString('hex');
	nmcli(
		'device',
		'wifi',
		'hotspot',
		'ifname',
		device,
		'con-name',
		CONNECTION_NAME,
		'ssid',
		ssid,
		'password',
		passphrase,
		'band',
		'bg',
	);
	const deadline = Date.now() + 20_000;
	for (;;) {
		const address = nmcli('-g', 'IP4.ADDRESS', 'device', 'show', device)
			.trim()
			.split('/')[0];
		if (address !== '') {
			console.log(`[hotspot] ${ssid} up on ${device} at ${address}`);
			return { ssid, passphrase, address };
		}
		if (Date.now() > deadline) {
			stopHotspot();
			throw new Error(`the hotspot came up but ${device} never got an address`);
		}
		await new Promise(resolve => setTimeout(resolve, 500));
	}
}

/** Drop the hotspot; the card reconnects to its usual network on its own. */
export function stopHotspot(): void {
	for (const action of ['down', 'delete']) {
		try {
			execFileSync('nmcli', ['connection', action, CONNECTION_NAME], {
				stdio: 'ignore',
			});
		} catch {
			/* not up, or already gone */
		}
	}
}
