/**
 * What keeps two runs on one LAN apart. Every e2e build of this checkout is
 * given this id: the agents' p2p network id derives from it, so connections
 * from another run's agents are refused, and the hubs' mDNS service name
 * carries it, so another run's hubs are never browsed. It is a hash of the
 * checkout's path: runs from different checkouts never share it, and a
 * rebuild in place keeps it, so turbo's caches stay valid.
 */
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

export const E2E_NETWORK_ID = createHash('sha256')
	.update(ROOT)
	.digest('hex')
	.slice(0, 8);

/** What a run claims to be the one run of this checkout: its data dir and
 *  its baked network id are the checkout's, so two runs of it cannot
 *  coexist and the second waits for the first. */
export const CHECKOUT_CLAIM = `checkout-${E2E_NETWORK_ID}`;
