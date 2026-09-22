/** What every spec that drives phones needs before it starts: the phones on a
 *  Wi-Fi network, and all of them on the same one. A run that was killed skips
 *  its own teardown, so it can leave a phone off Wi-Fi or parked on a lab
 *  network the fuzz walked it onto, and the next spec then fails somewhere far
 *  from the cause. */
import type { Agent } from './setup-agents';
import { wifiNetworks } from './test-env';

/** The phones a run can put on a network: physical devices only. An emulator
 *  has no real radio — its "Wi-Fi" is the NAT its own AVD provides, so every
 *  emulator reports the same SSID while being unreachable from any other, and
 *  asserting over that would only buy false confidence. */
function drivesWifi(agent: Agent): boolean {
	return agent.platform === 'android' || agent.platform === 'ios';
}

/**
 * Put every phone on a usable network and check they share it. A phone off
 * Wi-Fi gets its radio turned back on; one sitting on a lab network has that
 * network forgotten so the supplicant falls back to the saved home one. Then
 * every phone must name a network, and all of them the same network — two
 * phones on different LANs cannot see each other, which no spec that syncs
 * between them can survive, and which is far cheaper to say here than to
 * diagnose from a sync timeout later.
 */
export async function ensurePhonesShareALan(agents: Agent[]): Promise<void> {
	const phones = agents.filter(drivesWifi);
	if (phones.length === 0) return;
	// Each phone's radio is its own; doing them at once costs one association
	// wait rather than one per phone.
	const ssids = await Promise.all(phones.map(onHomeNetwork));
	if (new Set(ssids).size > 1) {
		const where = phones
			.map((phone, i) => `${label(phone, i)} on "${ssids[i]}"`)
			.join(', ');
		throw new Error(
			`the phones are on different networks (${where}); put them all on ` +
				'the network the host is on',
		);
	}
}

/** Get one phone onto its home network and answer with the network it ended up
 *  on. Throws naming the phone if it cannot be got onto one. */
async function onHomeNetwork(agent: Agent, index: number): Promise<string> {
	const lab = new Set(
		wifiNetworks()
			.filter(n => !n.home)
			.map(n => n.ssid),
	);
	let ssid = await agent.wifiSsid();
	if (ssid === '') {
		// `enableWifi` waits for an address, so what comes back is a network the
		// phone can actually use, not merely one it has associated with.
		await agent.enableWifi();
		ssid = await agent.wifiSsid();
	}
	if (lab.has(ssid)) {
		await agent.forgetWifi(ssid);
		ssid = await agent.wifiSsid();
	}
	if (ssid === '') {
		throw new Error(
			`${label(agent, index)} is on no Wi-Fi network and would not join ` +
				"one; check it has the host's network saved",
		);
	}
	return ssid;
}

function label(agent: Agent, index: number): string {
	return `${agent.platform} phone ${index + 1}`;
}
