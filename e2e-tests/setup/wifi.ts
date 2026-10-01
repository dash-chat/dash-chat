/** What every platform's Wi-Fi control has in common: the shape of a
 *  reading, and the wait for one. */
import { execFileSync } from 'node:child_process';
import os from 'node:os';

/** Comfortably longer than a WPA2 association plus DHCP on a busy 2.4GHz AP. */
export const WIFI_REASSOCIATE_MS = 90_000;

export interface WifiInfo {
	/** The SSID the device is associated with, or '' while it is on none, or
	 *  where the platform keeps it from the harness. */
	ssid: string;
	/** The device's IPv4 address on its Wi-Fi interface, or '' while it has
	 *  none. */
	address: string;
	/** The LAN that address is on, as `a.b.c.d/n` with the host bits cleared;
	 *  '' while there is no address. Two devices on one LAN read the same. */
	network: string;
}

/** The LAN `address` is on under a `prefixLength`-bit mask, as
 *  `a.b.c.d/n`; '' when either says there is no address. */
export function networkOf(address: string, prefixLength: number): string {
	if (address === '' || !(prefixLength > 0)) return '';
	const bits = address
		.split('.')
		.map(Number)
		.reduce((acc, octet) => ((acc << 8) | octet) >>> 0, 0);
	const mask = (0xffffffff << (32 - prefixLength)) >>> 0;
	const base = (bits & mask) >>> 0;
	return `${base >>> 24}.${(base >>> 16) & 255}.${(base >>> 8) & 255}.${base & 255}/${prefixLength}`;
}

/** How often the Wi-Fi state is re-read while waiting for it: callers time
 *  discovery from the moment the address appears, so the poll has to be fine
 *  next to the budget they hold it to. */
const WIFI_POLL_MS = 250;

/** Poll `read` until it answers non-empty, or throw `timeoutMsg` once
 *  WIFI_REASSOCIATE_MS have passed. */
export async function waitForWifi(
	read: () => string | Promise<string>,
	timeoutMsg: string,
): Promise<string> {
	const deadline = Date.now() + WIFI_REASSOCIATE_MS;
	for (;;) {
		const value = await read();
		if (value !== '') return value;
		if (Date.now() > deadline) throw new Error(timeoutMsg);
		await new Promise(resolve => setTimeout(resolve, WIFI_POLL_MS));
	}
}

/** Whether one ping from the host reaches `address`, given three tries: a
 *  phone that has just associated can miss the first. It is how a phone is
 *  known to be on the host's LAN — a test network, or any other offline one a
 *  phone may have saved, serves a subnet the host cannot reach. */
export function reachableFromHost(address: string): boolean {
	if (address === '') return false;
	for (let attempt = 1; attempt <= 3; attempt++) {
		try {
			execFileSync('ping', ['-c', '1', address], {
				stdio: 'ignore',
				timeout: 3_000,
			});
			return true;
		} catch {
			/* no answer yet */
		}
	}
	return false;
}

/** Whether a phone at `address` is on the host's network: 'host' when it
 *  shares a subnet with one of the host's interfaces and the host reaches it
 *  there, 'elsewhere' when neither holds, 'unsure' otherwise. The host's
 *  network is the one a phone must never forget, and forgetting cannot be
 *  undone, so only 'elsewhere' is grounds to forget a network the run does
 *  not list. Known by subnet rather than by name, which macOS keeps from a
 *  shell; the ping tells it from another network on the same subnet. */
export function hostNetworkVerdict(
	address: string,
): 'host' | 'elsewhere' | 'unsure' {
	const sharesSubnet = Object.values(os.networkInterfaces())
		.flat()
		.some(iface => {
			if (iface === undefined || iface.family !== 'IPv4') return false;
			if (iface.internal || iface.cidr === null) return false;
			const prefix = Number(iface.cidr.split('/')[1]);
			return networkOf(address, prefix) === networkOf(iface.address, prefix);
		});
	const reachable = reachableFromHost(address);
	if (sharesSubnet && reachable) return 'host';
	if (!sharesSubnet && !reachable) return 'elsewhere';
	return 'unsure';
}
