/**
 * What keeps two runs on one LAN apart. Every e2e build of this checkout is
 * given this id, and the mailbox and hubs it spawns are passed it: it is their
 * p2p network id, so connections from another run's agents are refused, and a
 * prefix of it suffixes the hubs' mDNS service name, so another run's hubs are
 * never browsed. It is a hash of the checkout's path: runs from different
 * checkouts never share it, and a rebuild in place keeps it, so turbo's caches
 * stay valid.
 */
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

export const E2E_NETWORK_ID = createHash('sha256').update(ROOT).digest('hex');

const SHORT_ID = E2E_NETWORK_ID.slice(0, 8);

/** What a run claims to be the one run of this checkout: its data dir and
 *  its baked network id are the checkout's, so two runs of it cannot
 *  coexist and the second waits for the first. */
export const CHECKOUT_CLAIM = `checkout-${SHORT_ID}`;

/** The ports this checkout's mailbox and push server prefer: stable across
 *  its runs, so the URLs baked into device builds stay valid and turbo's
 *  build skip fires; its own, so another checkout's run never takes a port
 *  a spec has released, as a cut cloud link does. */
const PORT_SLOT = parseInt(SHORT_ID, 16) % 1000;
export const MAILBOX_PREFERRED_PORT = 3300 + 2 * PORT_SLOT;
export const PUSH_PREFERRED_PORT = MAILBOX_PREFERRED_PORT + 1;
