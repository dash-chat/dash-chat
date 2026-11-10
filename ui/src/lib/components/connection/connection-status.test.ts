import type {
	AllMailboxesConnectionState,
	MailboxConnectionHealth,
} from 'dash-chat-stores';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
	connectedToCloudMailboxServer,
	connectionStatus,
	internetStatus,
	reachableLocalMailboxCount,
} from './connection-status';

const RESUMED_AT = Date.parse('2026-01-05T12:00:00Z');
const BEFORE_RESUME = '2026-01-05T11:59:00Z';
const AFTER_RESUME = '2026-01-05T12:00:01Z';
const SETTLED_AT = RESUMED_AT + 10_000;

function answering(at: string | null = AFTER_RESUME): MailboxConnectionHealth {
	return {
		status: 'Active',
		consecutive_errors: 0,
		next_poll_in_ms: 0,
		last_success_at: at,
		last_error: null,
	};
}

function failing(
	consecutiveErrors: number,
	at: string = AFTER_RESUME,
): MailboxConnectionHealth {
	return {
		status: 'Active',
		consecutive_errors: consecutiveErrors,
		next_poll_in_ms: 0,
		last_success_at: BEFORE_RESUME,
		last_error: { at, message: 'timed out' },
	};
}

function status(
	mailboxes: Partial<AllMailboxesConnectionState>,
	peerOnline = false,
) {
	return connectionStatus(
		{ cloudMailbox: null, localMailboxes: [], ...mailboxes },
		peerOnline,
		RESUMED_AT,
	);
}

beforeEach(() => {
	vi.useFakeTimers();
	vi.setSystemTime(RESUMED_AT + 1_000);
});

afterEach(() => {
	vi.useRealTimers();
});

describe('connectionStatus', () => {
	it('reads cloud once the cloud answered since the resume', () => {
		expect(status({ cloudMailbox: answering() })).toEqual({ status: 'cloud' });
	});

	it('reads pending while the cloud has only answered before the resume', () => {
		expect(status({ cloudMailbox: answering(BEFORE_RESUME) })).toEqual({
			status: 'pending',
		});
	});

	it('reads pending while a just-registered cloud has not answered yet', () => {
		expect(status({ cloudMailbox: answering(null) })).toEqual({
			status: 'pending',
		});
	});

	it('reads pending, not local, when the cloud failed a single poll', () => {
		expect(
			status({ cloudMailbox: failing(1), localMailboxes: [answering()] }),
		).toEqual({ status: 'pending' });
	});

	it('reads local with the reachable hub count once the cloud is down', () => {
		expect(
			status({
				cloudMailbox: failing(2),
				localMailboxes: [answering(), failing(3)],
			}),
		).toEqual({ status: 'local', count: 1 });
	});

	it('reads disconnected as soon as the cloud failed since the resume', () => {
		expect(status({ cloudMailbox: failing(2) })).toEqual({
			status: 'disconnected',
		});
	});

	it('reads p2p instead of disconnected while a peer is online', () => {
		expect(status({ cloudMailbox: failing(2) }, true)).toEqual({
			status: 'p2p',
		});
	});

	it('reads pending while settling on a failure from before the resume', () => {
		expect(status({ cloudMailbox: failing(2, BEFORE_RESUME) })).toEqual({
			status: 'pending',
		});
	});

	it('reads pending while settling on a cloud that never registered', () => {
		expect(status({})).toEqual({ status: 'pending' });
	});

	it('reads disconnected once settling ends without a verdict', () => {
		vi.setSystemTime(SETTLED_AT);
		expect(status({})).toEqual({ status: 'disconnected' });
	});
});

describe('internetStatus', () => {
	function internet(cloudMailbox: MailboxConnectionHealth | null) {
		return internetStatus({ cloudMailbox, localMailboxes: [] }, RESUMED_AT);
	}

	it('reads the cloud on its own, even with a local hub standing in', () => {
		expect(
			internetStatus(
				{ cloudMailbox: failing(3), localMailboxes: [answering()] },
				RESUMED_AT,
			),
		).toBe('disconnected');
	});

	it('is connected once a poll since resuming vouches for the cloud', () => {
		expect(internet(answering())).toBe('cloud');
	});

	it('stays pending until the settling window runs out', () => {
		expect(internet(null)).toBe('pending');
		vi.setSystemTime(SETTLED_AT);
		expect(internet(null)).toBe('disconnected');
	});
});

describe('reachableLocalMailboxCount', () => {
	it('counts only the hubs that are answering', () => {
		expect(
			reachableLocalMailboxCount({
				cloudMailbox: null,
				localMailboxes: [answering(), failing(3)],
			}),
		).toBe(1);
	});
});

describe('connectedToCloudMailboxServer', () => {
	it('counts a cloud below the error threshold as connected', () => {
		expect(
			connectedToCloudMailboxServer({
				cloudMailbox: failing(1),
				localMailboxes: [],
			}),
		).toBe(true);
	});

	it('does not count an unregistered or failing cloud', () => {
		expect(
			connectedToCloudMailboxServer({ cloudMailbox: null, localMailboxes: [] }),
		).toBe(false);
		expect(
			connectedToCloudMailboxServer({
				cloudMailbox: failing(2),
				localMailboxes: [],
			}),
		).toBe(false);
	});
});
