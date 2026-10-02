/**
 * Two phones keep syncing peer-to-peer as they hop between offline Wi-Fi
 * networks, together and apart, with address books crowded by contacts who
 * are gone for good.
 *
 * For every network in E2E_WIFI_NETWORKS, in order:
 *   1. a short-lived desktop visitor becomes a contact of both phones over
 *      the mailbox and is killed, leaving one more dead entry in each address
 *      book;
 *   2. both phones join the network and sync both ways;
 *   3. Alice moves on to the next network and writes to Bob, who can't be
 *      reached;
 *   4. Bob follows her there and must receive it, then they sync both ways;
 *   5. Bob drops off Wi-Fi, Alice writes again, and Bob must receive it once
 *      he is back.
 * The mailbox link is cut throughout 2–5: an Android phone reaches it over
 * USB whatever Wi-Fi it is on, so only a direct connection on a shared LAN
 * can carry anything. An iPhone reaches it over its home network alone, so it
 * goes back there to meet each visitor.
 *
 * Skips itself unless E2E_STRESS=1, two networks are configured (see
 * e2e-tests/.env.example) and the mailbox link can be cut. Run it with:
 *   PLATFORMS=android,android E2E_STRESS=1 just e2e run lan-network-hopping-stress
 *
 * Tunables: E2E_HOP_TOURS (how many times to walk every network, default 1).
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	type NamedAgent,
	expectSyncBothWays,
	meetVisitor,
	openChatWith,
	withMailboxCut,
} from '../helpers/flows/lan-sync';
import { envInt } from '../helpers/utils';
import { mailboxDegradable } from '../setup/mailbox-control';
import { ensurePhonesShareALan } from '../setup/phone-lan';
import { setupAgents } from '../setup/setup-agents';
import { wifiNetworks } from '../setup/test-env';

const TOURS = envInt('E2E_HOP_TOURS', 1);

/** A visitor, three network moves and four exchanges per network. */
const PER_NETWORK_MS = 8 * 60 * 1_000;

// wdio arms its per-hook abort timer from the mocha timeout at invocation time,
// so this has to be set suite-wide rather than inside a hook or test body.
describe('LAN sync while hopping networks', function () {
	const networks = wifiNetworks();
	this.timeout(TOURS * Math.max(networks.length, 1) * PER_NETWORK_MS);

	let alice: NamedAgent;
	let bob: NamedAgent;
	/** The network both phones are on when the run starts. */
	let home: string;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (networks.length < 2) this.skip();
		if (!mailboxDegradable()) this.skip();
		const [agent1, agent2] = await setupAgents(this, [
			{ platform: 'phone' },
			{ platform: 'phone' },
		]);
		home = await agent1.wifiSsid();
		alice = { agent: agent1, name: 'Alice' };
		bob = { agent: agent2, name: 'Bob' };
		await createProfiles({ Alice: agent1, Bob: agent2 });
		await exchangeContacts([agent1, agent2]);
	});

	after(async () => {
		if (alice === undefined) return;
		await Promise.all([alice, bob].map(leaveLabNetworks));
		await ensurePhonesShareALan([alice.agent, bob.agent]);
	});

	async function backToMailbox(phone: NamedAgent): Promise<void> {
		if (phone.agent.platform !== 'ios') return;
		// Saved on the phone, so it joins without its passphrase.
		await phone.agent.connectWifi(home, '');
	}

	/** Forgetting only the network a phone is on drops it onto the next lab
	 *  network it has saved, so every one of them goes. */
	async function leaveLabNetworks(phone: NamedAgent): Promise<void> {
		if ((await phone.agent.wifiSsid()) === '') await phone.agent.enableWifi();
		await backToMailbox(phone);
		for (const network of networks) {
			if (network.ssid !== home) await phone.agent.forgetWifi(network.ssid);
		}
	}

	it('syncs on every network, apart and reunited', async () => {
		let visitors = 0;
		for (let tour = 1; tour <= TOURS; tour++) {
			for (let i = 0; i < networks.length; i++) {
				const here = networks[i];
				const next = networks[(i + 1) % networks.length];
				const step = `tour ${tour}, ${here.ssid}`;

				visitors++;
				await Promise.all([alice, bob].map(backToMailbox));
				await meetVisitor(visitors, [alice, bob]);

				await withMailboxCut(async () => {
					await Promise.all([
						alice.agent.connectWifi(here.ssid, here.passphrase),
						bob.agent.connectWifi(here.ssid, here.passphrase),
					]);
					await expectSyncBothWays(alice, bob, `together on ${step}`);

					await alice.agent.connectWifi(next.ssid, next.passphrase);
					const stranded = `Sent from ${next.ssid} before Bob came (${step})`;
					await send(alice, bob, stranded);
					await bob.agent.connectWifi(next.ssid, next.passphrase);
					await bob.agent.directChatPage.messages.waitForMessage(stranded);
					await expectSyncBothWays(
						alice,
						bob,
						`reunited on ${next.ssid}, ${step}`,
					);

					await bob.agent.disableWifi();
					const missed = `Sent while Bob was off Wi-Fi (${step})`;
					await send(alice, bob, missed);
					await bob.agent.connectWifi(next.ssid, next.passphrase);
					await bob.agent.directChatPage.messages.waitForMessage(missed);
				});
				console.log(`[lan-network-hopping] ${step} synced`);
			}
		}
	});
});

async function send(
	from: NamedAgent,
	to: NamedAgent,
	text: string,
): Promise<void> {
	await openChatWith(from.agent, to.name);
	await from.agent.directChatPage.composer.sendMessage(text);
}
