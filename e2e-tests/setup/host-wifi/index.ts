/**
 * The host's Wi-Fi card as a client of the networks a run may use: joining
 * one is what puts the hubs on that LAN, since every hub is a process on
 * the host. The host keeps its wired link for everything else, and the card
 * goes back to its usual network once the run lets it go. The platform's
 * own tooling does the work: NetworkManager on Linux, `networksetup` on
 * macOS.
 */
import { claimDeviceWhenFree, releaseDevice } from '../device-lock';
import { linux } from './linux';
import { macos } from './macos';

const driver = process.platform === 'darwin' ? macos : linux;

export const { wifiDevice, visibleNetworks } = driver;

/** The card this spec file has claimed, until [`releaseWifiDevice`]. */
let claimed: string | null = null;

/** Claim `device` for this spec file before moving it: the card is the one
 *  thing runs on a host cannot share, so a spec that joins or leaves with it
 *  waits for whichever run is moving it now, and keeps it until it ends. */
async function claimFirst(device: string): Promise<void> {
	if (claimed === device) return;
	await claimDeviceWhenFree(device);
	claimed = device;
}

export async function joinWifi(
	device: string,
	ssid: string,
	passphrase: string,
): Promise<string> {
	await claimFirst(device);
	return await driver.joinWifi(device, ssid, passphrase);
}

/** Leave `ssid`, which takes the card off it: another run's card too, were
 *  it on that network, so this claims the card like a join does. */
export async function leaveWifi(ssid: string): Promise<void> {
	const device = wifiDevice();
	if (device !== null) await claimFirst(device);
	await driver.leaveWifi(ssid);
}

/** Give the card back at the end of the spec file. */
export function releaseWifiDevice(): void {
	if (claimed === null) return;
	releaseDevice(claimed);
	claimed = null;
}

/** Fails a run before its first join when `device` cannot see every one of
 *  `ssids`, instead of timing out on that join move after move. */
export function assertInRange(device: string, ssids: string[]): void {
	const visible = visibleNetworks(device);
	if (visible === null) {
		console.log(`[wifi] ${device} will not list networks; joining blind`);
		return;
	}
	const missing = ssids.filter(ssid => !visible.includes(ssid));
	if (missing.length === 0) return;
	const quote = (list: string[]) => list.map(s => `"${s}"`).join(', ');
	throw new Error(
		`${device} cannot see ${quote(missing)}; it sees ${quote(visible)}`,
	);
}
