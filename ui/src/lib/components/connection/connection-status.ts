import { withinWindow } from '$lib/utils/time';
import type {
	AllMailboxesConnectionState,
	MailboxConnectionState,
} from 'dash-chat-stores';

/** How long a verdict may stay unproven after the page becomes visible before
 * it is shown anyway. A reachable mailbox registers and completes a poll well
 * inside this, so the wait only runs out when something really is wrong —
 * without it, a cloud mailbox that has never been reachable would produce no
 * measurement at all and the user would be told nothing, forever. */
const SETTLING_MS = 10_000;

// Call a mailbox down after this many consecutive errors. Intentionally lower
// than the backend's degraded_threshold (5) so the UI reacts faster than the
// connection state alone would suggest.
const DISCONNECTED_ERROR_THRESHOLD = 2;

export type ConnectionStatus =
	| { status: 'cloud' }
	| { status: 'local'; count: number }
	| { status: 'p2p' }
	| { status: 'disconnected' }
	| { status: 'pending' };

function reachable(mailbox: MailboxConnectionState): boolean {
	return (
		mailbox.status === 'Active' &&
		mailbox.consecutive_errors < DISCONNECTED_ERROR_THRESHOLD
	);
}

function happenedAfter(at: string | null | undefined, instant: number) {
	return at !== null && at !== undefined && Date.parse(at) > instant;
}

export function connectedToCloudMailboxServer(
	mailboxes: AllMailboxesConnectionState,
): boolean {
	return mailboxes.cloudMailbox !== null && reachable(mailboxes.cloudMailbox);
}

/** What the user is told about their connection: 'pending' while no verdict
 * has been earned yet.
 *
 * The cloud takes precedence: a local hub is only worth mentioning while it
 * stands in for an unreachable cloud, so a cloud that is still reachable but
 * not vouched for reads pending even with a hub up. Vouching takes a poll
 * answered since `resumedAt` with none failed after it; anything measured
 * before `resumedAt` happened while the app was away and says nothing about
 * the connection now, so neither a stale success nor a stale failure settles
 * the verdict. Reactive through `withinWindow`, so the settling deadline
 * re-renders the caller when it expires. */
export function connectionStatus(
	mailboxes: AllMailboxesConnectionState,
	peerOnline: boolean,
	resumedAt: number,
): ConnectionStatus {
	const { cloudMailbox } = mailboxes;
	if (cloudMailbox !== null && reachable(cloudMailbox)) {
		const vouched =
			cloudMailbox.consecutive_errors === 0 &&
			happenedAfter(cloudMailbox.last_success_at, resumedAt);
		return { status: vouched ? 'cloud' : 'pending' };
	}
	const count = mailboxes.localMailboxes.filter(reachable).length;
	if (count > 0) return { status: 'local', count };
	const failedSinceResume = happenedAfter(
		cloudMailbox?.last_error?.at,
		resumedAt,
	);
	if (!failedSinceResume && withinWindow(resumedAt, SETTLING_MS))
		return { status: 'pending' };
	return { status: peerOnline ? 'p2p' : 'disconnected' };
}
