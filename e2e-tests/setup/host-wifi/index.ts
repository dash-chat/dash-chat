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

export const { wifiDevice, joinWifi, leaveWifi } =
	process.platform === 'darwin' ? macos : linux;
