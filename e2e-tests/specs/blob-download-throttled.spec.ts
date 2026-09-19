/**
 * FOLLOW-UP, not yet runnable: a large attachment arriving over a slow cloud
 * link. The mailbox serves blob bytes no faster than a budget, so the
 * receiver's progress ring fills a little at every poll, and a mailbox that
 * stops answering mid-download leaves the progress where it was until it
 * answers again, after which it climbs on.
 *
 * What this needs, and what the follow-up PR to the pull-based progress work
 * brings:
 *
 * 1. A test-only throttle on the mailbox's blob provider (behind the
 *    mailbox-server `test_utils` feature, so production mailboxes never
 *    intercept a chunk), exposed as `POST /testing/blob-throttle` and driven
 *    from the harness by `setMailboxBlobThrottle()` in
 *    `e2e-tests/setup/mailbox-control.ts`. The run's toxiproxy link cannot
 *    stand in for the slow link: blobs travel over iroh's QUIC (UDP)
 *    connection, not the mailbox's HTTP port.
 * 2. The e2e mailbox serving blobs to e2e apps at all: the app's e2e network
 *    id and the mailbox's `--network-id` have to derive the same blob ALPN.
 *
 * Until then the suite is skipped, and each case fails on purpose if run.
 */
const FOLLOW_UP =
	'Needs the mailbox blob throttle from the follow-up PR; see the header of this spec.';

describe.skip('Blob download over a slow cloud link', function () {
	this.timeout(300_000);

	it('fills the progress ring at every poll while the mailbox serves slowly', async () => {
		// send('slow.bin', 640 KiB); wait for the first byte; expect the ring's
		// aria-valuenow to be non-decreasing across polls with a net increase;
		// expect the ring to go away once complete.
		throw new Error(FOLLOW_UP);
	});

	it('holds the progress while the mailbox is away and climbs again once it is back', async () => {
		// send('flaky.bin', 768 KiB); wait for the first byte; suspendMailbox();
		// expect the reading unchanged after a settle; resumeMailbox(); expect it
		// to climb again and complete.
		throw new Error(FOLLOW_UP);
	});
});
