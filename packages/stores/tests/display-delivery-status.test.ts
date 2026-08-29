import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	UNSENT_WINDOW_MS,
	displayDeliveryStatus,
} from '../src/message-acks/display-delivery-status';

const SENT_AT = new Date(2026, 0, 5, 12, 0, 0).valueOf();

describe('displayDeliveryStatus', () => {
	it("shows 'unsent' while a sending message is younger than the window", () => {
		assert.equal(displayDeliveryStatus('sending', SENT_AT, SENT_AT), 'unsent');
		assert.equal(
			displayDeliveryStatus('sending', SENT_AT, SENT_AT + UNSENT_WINDOW_MS - 1),
			'unsent',
		);
	});

	it("downgrades to 'sending' once the window has elapsed", () => {
		assert.equal(
			displayDeliveryStatus('sending', SENT_AT, SENT_AT + UNSENT_WINDOW_MS),
			'sending',
		);
	});

	it('passes non-sending statuses through regardless of age', () => {
		assert.equal(displayDeliveryStatus('mailbox', SENT_AT, SENT_AT), 'mailbox');
		assert.equal(
			displayDeliveryStatus('delivered', SENT_AT, SENT_AT),
			'delivered',
		);
		assert.equal(
			displayDeliveryStatus('mailbox', SENT_AT, SENT_AT + UNSENT_WINDOW_MS * 2),
			'mailbox',
		);
	});

	it('treats a clock-skewed future timestamp as within the window', () => {
		assert.equal(
			displayDeliveryStatus('sending', SENT_AT + 60_000, SENT_AT),
			'unsent',
		);
	});
});
