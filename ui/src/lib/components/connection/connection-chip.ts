import { withinWindow } from '$lib/utils/time';

/** How long a verdict may stay unproven after the page becomes visible before
 * it is shown anyway. A reachable mailbox registers and completes a poll well
 * inside this, so the wait only runs out when something really is wrong —
 * without it, a cloud mailbox that has never been reachable would produce no
 * measurement at all and the user would be told nothing, forever. */
const SETTLING_MS = 10_000;

interface Connection {
	connectedToCloudMailboxServer: boolean;
	cloudFailedLastPoll: boolean;
	/** When the cloud mailbox last failed a poll, or null if it never has. */
	cloudLastFailureAtMs: number | null;
	connectedLocalMailboxCount: number;
}

export type ConnectionChipStatus =
	| 'cloud'
	| 'local'
	| 'p2p'
	| 'disconnected'
	| 'pending';

/** What the chip reads: 'pending' while no verdict has been earned yet.
 *
 * The cloud takes precedence: a local hub is only worth mentioning while it
 * stands in for an unreachable cloud. A connection, cloud or hub, is a fact
 * and shows at once, but a cloud whose latest poll failed is no longer
 * vouched for, so it reads pending until a poll answers or enough fail to
 * call it down. Saying the cloud is down, even with an online peer standing
 * in, is gated on freshness: a failure recorded before `resumedAt` happened
 * while the app was away and says nothing about the connection now. Reactive
 * through `withinWindow`, so the settling deadline re-renders the caller when
 * it expires. */
export function connectionChipStatus(
	connection: Connection,
	peerOnline: boolean,
	resumedAt: number,
): ConnectionChipStatus {
	if (
		connection.connectedToCloudMailboxServer &&
		!connection.cloudFailedLastPoll
	)
		return 'cloud';
	if (connection.connectedLocalMailboxCount > 0) return 'local';
	if (connection.connectedToCloudMailboxServer) return 'pending';
	const failedSinceResume =
		connection.cloudLastFailureAtMs !== null &&
		connection.cloudLastFailureAtMs > resumedAt;
	if (!failedSinceResume && withinWindow(resumedAt, SETTLING_MS))
		return 'pending';
	return peerOnline ? 'p2p' : 'disconnected';
}
