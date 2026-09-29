/** What every spec that drives phones needs before it starts: every phone on
 *  a network of its own, all of them on the same one, with internet. A run
 *  that was killed skips its own teardown, so it can leave a phone off Wi-Fi
 *  or parked on a lab network the fuzz walked it onto, and the next spec then
 *  fails somewhere far from the cause. This drives the phones there from any
 *  starting state instead of asserting they were left there. */
import { execFileSync } from 'node:child_process';

import type { Agent } from './setup-agents';
import type { WifiInfo } from './wifi';

/** The phones a run can put on a network: physical devices only. An emulator
 *  has no real radio — its "Wi-Fi" is the NAT its own AVD provides, so every
 *  emulator reports the same SSID while being unreachable from any other, and
 *  asserting over that would only buy false confidence. */
function drivesWifi(agent: Agent): boolean {
	return agent.platform === 'android' || agent.platform === 'ios';
}

/**
 * Put every phone on a network of its own and check they share it. Every lab
 * network saved on a phone is forgotten, so the supplicant falls back to the
 * network the user saved; a phone left off the air gets its radio turned
 * back on. Then every phone must hold an address, all on one subnet — two
 * phones on different LANs cannot see each other, which no spec that syncs
 * between them can survive, and which is far cheaper to say here than to
 * diagnose from a sync timeout later — and the host must reach every phone,
 * since two networks can serve the same subnet.
 */
export async function convergePhoneNetworks(agents: Agent[]): Promise<void> {
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

/** Get one phone onto a network of its own — lab networks forgotten, radio
 *  on — and answer with where it is; a failure names the phone. */
async function onOwnNetwork(agent: Agent, index: number): Promise<WifiInfo> {
	try {
		return await agent.leaveWifi();
	} catch (err) {
		throw new Error(`${label(agent, index)}: ${String(err)}`);
	}
}

/** Whether one ping from the host reaches `address`, given three tries:
 *  a phone that has just associated can miss the first. */
function reachableFromHost(address: string): boolean {
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

function label(agent: Agent, index: number): string {
	return `${agent.platform} phone ${index + 1}`;
}
