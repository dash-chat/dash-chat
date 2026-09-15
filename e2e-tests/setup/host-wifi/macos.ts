/** `networksetup` does the joining; the card is put back on its usual
 *  network by forgetting the lab network and cycling its power, after which
 *  it autoconnects on its own. macOS does not let a shell read which network
 *  the card is on, so nothing here ever asks — and only a network this
 *  process joined is ever forgotten, so the usual one cannot be. */
import { execFileSync } from 'node:child_process';

import { type HostWifi, waitForAddress } from './driver';

function networksetup(...args: string[]): string {
	return execFileSync('networksetup', args, { encoding: 'utf8' });
}

/** Join `ssid` on `device`. `networksetup` reports failure as a line of
 *  output and exits 0 either way, so silence is success. */
function join(device: string, ssid: string, passphrase: string): void {
	const output = networksetup(
		'-setairportnetwork',
		device,
		ssid,
		...(passphrase === '' ? [] : [passphrase]),
	).trim();
	if (output !== '') throw new Error(output);
}

/** The networks this process joined, the only ones it may forget. */
const joinedNetworks = new Set<string>();

function address(device: string): string {
	try {
		return execFileSync('ipconfig', ['getifaddr', device], {
			encoding: 'utf8',
		}).trim();
	} catch {
		return '';
	}
}

function wifiDevice(): string | null {
	const ports = networksetup('-listallhardwareports').split('\n');
	const wifi = ports.findIndex(line => line === 'Hardware Port: Wi-Fi');
	if (wifi === -1) return null;
	const device = /^Device: (.+)$/.exec(ports[wifi + 1] ?? '');
	return device === null ? null : device[1];
}

interface AirPortNetwork {
	_name: string;
}

interface AirPortInterface {
	_name: string;
	spairport_current_network_information?: AirPortNetwork;
	spairport_airport_other_local_wireless_networks?: AirPortNetwork[];
}

interface AirPortReport {
	SPAirPortDataType: { spairport_airport_interfaces: AirPortInterface[] }[];
}

/** Since macOS 15.6 the report names networks `<redacted>` unless the
 *  caller holds location access, which a shell does not. */
function visibleNetworks(device: string): string[] | null {
	const report = JSON.parse(
		execFileSync('system_profiler', ['SPAirPortDataType', '-json'], {
			encoding: 'utf8',
		}),
	) as AirPortReport;
	const iface = report.SPAirPortDataType.flatMap(
		data => data.spairport_airport_interfaces,
	).find(i => i._name === device);
	if (iface === undefined) return [];
	const current = iface.spairport_current_network_information;
	const networks = [
		...(current === undefined ? [] : [current]),
		...(iface.spairport_airport_other_local_wireless_networks ?? []),
	];
	const ssids = [...new Set(networks.map(n => n._name))];
	return ssids.includes('<redacted>') ? null : ssids;
}

function leaveWifi(ssid: string): void {
	const device = wifiDevice();
	if (device === null) return;
	if (joinedNetworks.delete(ssid)) {
		networksetup('-removepreferredwirelessnetwork', device, ssid);
	}
	networksetup('-setairportpower', device, 'off');
	networksetup('-setairportpower', device, 'on');
	console.log(`[wifi] ${device} left ${ssid}`);
}

export const macos: HostWifi = {
	wifiDevice,
	visibleNetworks,

	async joinWifi(device, ssid, passphrase) {
		// The card scans for the network itself, but a scan right after
		// leaving another network can miss it.
		const joinBy = Date.now() + 60_000;
		for (;;) {
			try {
				join(device, ssid, passphrase);
				joinedNetworks.add(ssid);
				break;
			} catch (err) {
				if (Date.now() > joinBy) {
					throw new Error(
						`could not join "${ssid}" on ${device}: ${String(err)}`,
					);
				}
				await new Promise(resolve => setTimeout(resolve, 3_000));
			}
		}
		const joined = await waitForAddress(
			() => address(device),
			() => leaveWifi(ssid),
			`joined "${ssid}" but ${device} never got an address`,
		);
		console.log(`[wifi] ${device} joined ${ssid} at ${joined}`);
		return joined;
	},

	leaveWifi,
};
