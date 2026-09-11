import { withinWindow } from '$lib/utils/time';

/** How long a verdict may stay unproven after the page becomes visible before
 * it is shown anyway. A reachable mailbox registers and completes a poll well
 * inside this, so the wait only runs out when something really is wrong —
 * without it, a cloud mailbox that has never been reachable would produce no
 * measurement at all and the user would be told nothing, forever. */
const SETTLING_MS = 10_000;

interface Connection {
	connectedToCloudMailboxServer: boolean;
	/** When the cloud mailbox last failed a poll, or null if it never has. */
	cloudLastFailureAtMs: number | null;
	connectedLocalMailboxCount: number;
}

export type ConnectionChipStatus = 'local' | 'disconnected';

/** What the chip reads, or null to leave it off screen.
 *
 * The chip only has something to say while the cloud is out of reach: that a
 * local hub is standing in, or that nothing is. A connected hub is a fact and
 * shows at once. Accusing the connection of being down is gated on freshness:
 * a failure recorded before `resumedAt` happened while the app was away and
 * says nothing about the connection now, whereas erring towards "fine" on a
 * stale success costs the user nothing. Reactive through `withinWindow`, so
 * the settling deadline re-renders the caller when it expires. */
export function connectionChipStatus(
	connection: Connection,
	resumedAt: number,
): ConnectionChipStatus | null {
	if (connection.connectedToCloudMailboxServer) return null;
	if (connection.connectedLocalMailboxCount > 0) return 'local';
	const failedSinceResume =
		connection.cloudLastFailureAtMs !== null &&
		connection.cloudLastFailureAtMs > resumedAt;
	return failedSinceResume || !withinWindow(resumedAt, SETTLING_MS)
		? 'disconnected'
		: null;
}
