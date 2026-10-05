/** What every spec that drives phones needs before it starts: every phone on
 *  a network of its own, all of them on the same one, with internet. A run
 *  that was killed skips its own teardown, so it can leave a phone off Wi-Fi
 *  or parked on a test network the fuzz walked it onto, and the next spec then
 *  fails somewhere far from the cause. This drives the phones there from any
 *  starting state instead of asserting they were left there. */
import { leaveTestNetworks } from './host-wifi';
import type { Agent } from './setup-agents';
import { testNetworkSsids } from './test-env';
import { type WifiInfo, reachableFromHost } from './wifi';

/** The phones a run can put on a network: physical devices only. An emulator
 *  has no real radio — its "Wi-Fi" is the NAT its own AVD provides, so every
 *  emulator reports the same SSID while being unreachable from any other, and
 *  asserting over that would only buy false confidence. */
function drivesWifi(agent: Agent): boolean {
	return agent.platform === 'android' || agent.platform === 'ios';
}

/**
 * Put the host's card back on its usual network, then every phone on the
 * network the host is on. The card goes first, so a test network is never
 * the host's. Each phone forgets every test network, then every other network
 * it lands on that is clearly not the host's; one that may be the host's is
 * kept, and a phone left off the air gets its radio turned back on. Then
 * all of them must be on one subnet — two phones on different LANs cannot
 * see each other, which no spec that syncs between them can survive, and
 * which is far cheaper to say here than to diagnose from a sync timeout
 * later — and the host must reach every phone, since two networks can serve
 * the same subnet.
 */
export async function convergeNetworks(agents: Agent[]): Promise<void> {
	await leaveTestNetworks(testNetworkSsids());
	const phones = agents.filter(drivesWifi);
	if (phones.length === 0) return;
	// Each phone's radio is its own; doing them at once costs one association
	// wait rather than one per phone.
	const infos = await Promise.all(phones.map(onOwnNetwork));
	if (new Set(infos.map(info => info.network)).size > 1) {
		const where = phones
			.map((phone, i) => `${label(phone, i)} at ${infos[i].address}`)
			.join(', ');
		throw new Error(
			`the phones are on different networks (${where}); put them all on ` +
				'the network the host is on',
		);
	}
	// Two networks can hand out the same subnet — most home routers default
	// to one — so the host, which is on the network the phones must share,
	// has to be able to reach each of them.
	const unreachable = phones
		.map((phone, i) =>
			reachableFromHost(infos[i].address)
				? null
				: `${label(phone, i)} at ${infos[i].address}`,
		)
		.filter(name => name !== null);
	if (unreachable.length > 0) {
		throw new Error(
			`the host cannot reach ${unreachable.join(', ')}; is it on the host's network?`,
		);
	}
}

/** Drop every test network from every phone without waiting for where it
 *  lands: each falls back to the Wi-Fi its user saved on its own. */
export async function forgetTestNetworks(agents: Agent[]): Promise<void> {
	await Promise.allSettled(
		agents.filter(drivesWifi).map(agent => agent.forgetWifi()),
	);
}

/** Get one phone onto the host's LAN and answer with where it is; a failure
 *  names the phone. */
async function onOwnNetwork(agent: Agent, index: number): Promise<WifiInfo> {
	try {
		return await agent.leaveWifi();
	} catch (err) {
		throw new Error(`${label(agent, index)}: ${String(err)}`);
	}
}

function label(agent: Agent, index: number): string {
	return `${agent.platform} phone ${index + 1}`;
}
