/**
 * Drive the local mailbox toggle exactly like a user would: navigate to
 * Settings → Offline, click the toggle, and wait for the input to reflect
 * the new state. The agent ends up where it started.
 */
import { waitFor } from '../helpers';
import { S } from '../selectors';

const TOGGLE_INPUT = `${S.offline.localMailboxToggle} input[type="checkbox"]`;

function readToggleInput(): HTMLInputElement {
	const input = document.querySelector(TOGGLE_INPUT) as HTMLInputElement | null;
	if (!input) {
		throw new Error('setLocalMailboxEnabled: toggle input not found');
	}
	return input;
}

async function waitForToggleState(
	expected: boolean,
	timeout = 15_000,
): Promise<void> {
	const start = Date.now();
	for (;;) {
		if (readToggleInput().checked === expected) return;
		if (Date.now() - start > timeout) {
			throw new Error(
				`setLocalMailboxEnabled: toggle did not reach ${expected} within ${timeout}ms`,
			);
		}
		await new Promise(r => setTimeout(r, 50));
	}
}

export async function setLocalMailboxEnabled(enabled: boolean): Promise<void> {
	const returnPath = window.location.pathname;
	await window.__test.goto('/settings/offline');
	// The Toggle is inside `{#await $localMailboxEnabled then enabled}`, so it
	// renders only once the settings store has loaded.
	await waitFor(TOGGLE_INPUT);
	if (readToggleInput().checked !== enabled) {
		readToggleInput().click();
		await waitForToggleState(enabled);
	}
	if (returnPath && returnPath !== '/settings/offline') {
		await window.__test.goto(returnPath);
	}
}
