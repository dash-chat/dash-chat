import { allocatePinnedPortFrom } from './allocate-port';
import { killLeftoverRelay } from './cleanup';
import { lanHostAddress } from './host-ip';
import { phonePlatforms, remoteMailboxUrl } from './test-env';

/** The public relay a run against a remote mailbox uses: that mailbox's iroh
 *  endpoint is only reachable through it. */
const PUBLIC_RELAY_URL = 'https://e90mqanwh7cec3zww.aps1.relay.iroh-svc.com/';

/** The port the run's own relay (relay-local-server) serves on, when the run
 *  has a local mailbox. Baked into every build, so it stays the same from run
 *  to run unless something else holds it: past the 3300..5299 range a
 *  checkout's mailbox and push ports come from (network-id.ts). */
export function localRelayPort(): number {
	// A relay an interrupted run left behind would push this run onto the next
	// port, and the run after back again, each a rebuild of every app.
	if (
		process.env._WDIO_RELAY_PORT === undefined &&
		process.env.WDIO_WORKER_ID === undefined
	)
		killLeftoverRelay();
	return allocatePinnedPortFrom('_WDIO_RELAY_PORT', 5400);
}

/** The iroh relay e2e builds of the app and mailbox server bake in place of
 *  the production one (`RELAY_URL` in dashchat-utils). Every agent bakes the
 *  same one, since a node registers its peers under its own relay: with a
 *  local mailbox it is the run's own relay at the host's LAN address, which
 *  phones and desktops alike reach, so a spec can take it down. That relay is
 *  plain HTTP: only runs against a remote mailbox go through an HTTPS relay,
 *  as production does. */
export const E2E_RELAY_URL = pinnedRelayUrl();

/** Worked out once, by the launcher, and pinned in the env its workers
 *  inherit: a worker working it out again could land on another address. It
 *  only holds for the network the host was on at build time: a spec that moves
 *  the host's Wi-Fi leaves the relay unreachable for the rest of the run. */
function pinnedRelayUrl(): string {
	if (remoteMailboxUrl() !== null) return PUBLIC_RELAY_URL;
	if (process.env._WDIO_RELAY_URL === undefined) {
		const host = lanHostAddress();
		// Phones reach the relay at the host's LAN address, with no adb reverse
		// in between: on loopback they would have none.
		if (host === null && phonePlatforms().length > 0) {
			throw new Error(
				'No private LAN IPv4 for the phones to reach the relay on — set E2E_HOST_IP.',
			);
		}
		process.env._WDIO_RELAY_URL = `http://${host?.address ?? '127.0.0.1'}:${localRelayPort()}/`;
		console.log(
			`[relay] baking ${process.env._WDIO_RELAY_URL} (${host?.name ?? 'no LAN address'})`,
		);
	}
	return process.env._WDIO_RELAY_URL;
}
