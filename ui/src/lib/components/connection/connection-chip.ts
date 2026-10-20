import { withinWindow } from '$lib/utils/time';

import type { ConnectionChipStatus } from './connection-chip-status';

/** How long a verdict may stay unproven after the page becomes visible before
 * it is shown anyway. A reachable mailbox registers and completes a poll well
 * inside this, so the wait only runs out when something really is wrong —
 * without it, a cloud mailbox that has never been reachable would produce no
 * measurement at all and the user would be told nothing, forever. */
const SETTLING_MS = 10_000;

interface Connection {
	connectedToCloudMailboxServer: boolean;
	cloudFailedLastPoll: boolean;
	/** When the cloud mailbox last answered a poll, or null if it never has. */
	cloudLastSuccessAtMs: number | null;
	/** When the cloud mailbox last failed a poll, or null if it never has. */
	cloudLastFailureAtMs: number | null;
	connectedLocalMailboxCount: number;
}

/** What the chip reads: 'pending' while no verdict has been earned yet.
 *
 * The cloud takes precedence: a local hub is only worth mentioning while it
 * stands in for an unreachable cloud, so a cloud that is still counted as
 * connected but not vouched for reads pending even with a hub up. Vouching
 * takes a poll answered since `resumedAt` with none failed after it; anything
 * measured before `resumedAt` happened while the app was away and says nothing
 * about the connection now, so neither a stale success nor a stale failure
 * settles the verdict. Reactive through `withinWindow`, so the settling
 * deadline re-renders the caller when it expires. */
export function connectionChipStatus(
	connection: Connection,
	peerOnline: boolean,
	resumedAt: number,
): ConnectionChipStatus {
	if (connection.connectedToCloudMailboxServer) {
		const answeredSinceResume =
			connection.cloudLastSuccessAtMs !== null &&
			connection.cloudLastSuccessAtMs > resumedAt;
		return answeredSinceResume && !connection.cloudFailedLastPoll
			? 'cloud'
			: 'pending';
	}
	if (connection.connectedLocalMailboxCount > 0) return 'local';
	const failedSinceResume =
		connection.cloudLastFailureAtMs !== null &&
		connection.cloudLastFailureAtMs > resumedAt;
	if (!failedSinceResume && withinWindow(resumedAt, SETTLING_MS))
		return 'pending';
	return peerOnline ? 'p2p' : 'disconnected';
}
