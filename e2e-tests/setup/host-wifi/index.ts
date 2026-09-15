/**
 * The host's Wi-Fi card as a client of the networks a run may use: joining
 * one is what puts the hubs on that LAN, since every hub is a process on
 * the host. The host keeps its wired link for everything else, and the card
 * goes back to its usual network once the run lets it go. The platform's
 * own tooling does the work: NetworkManager on Linux, `networksetup` on
 * macOS.
 */
import { linux } from './linux';
import { macos } from './macos';

export const { wifiDevice, visibleNetworks, joinWifi, leaveWifi } =
	process.platform === 'darwin' ? macos : linux;

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
