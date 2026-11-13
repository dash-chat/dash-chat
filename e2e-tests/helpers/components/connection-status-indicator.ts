import type {
	ConnectionModeStatus,
	ConnectionStatusTag as ConnectionStatus,
} from '../../../ui/src/lib/components/connection/connection-status-tags';
import type { ConnectionStatusSample } from '../../../ui/tests/setup-utils';
import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export type { ConnectionModeStatus, ConnectionStatus, ConnectionStatusSample };

/** `status(visibility)@+Nms`, relative to the first sample — the absolute epoch
 *  values are unreadable in a failure message, and what matters is the spacing
 *  and whether anyone was looking. */
export function formatStatusTrace(samples: ConnectionStatusSample[]): string {
	const start = samples[0]?.at ?? 0;
	return samples
		.map(s => `${s.status}(${s.visibility})@+${s.at - start}ms`)
		.join(' -> ');
}

/** How long after the page becomes visible a rendered status can still be
 *  blamed on the time away. Inherited verdicts are painted as the queued state
 *  is processed — measured at 0ms and 75ms — while a newly earned one needs
 *  three consecutive failures, which has never been seen sooner than 5s. This
 *  sits an order of magnitude above the former and well below the latter. */
const RESUME_PAINT_MS = 2_000;

/** The samples the user could have seen as the app came back: from the moment
 *  the page became visible until a fresh verdict could plausibly have been
 *  earned. Anything rendered while hidden was shown to nobody, and anything
 *  rendered well after the return reflects the connection they have now.
 *
 *  Returns null when the page never reported becoming visible, which is not the
 *  same as "nothing was shown": it means the webview never fired
 *  `visibilitychange`, so this cannot be judged and must not read as a pass. */
export function samplesOnResume(
	samples: ConnectionStatusSample[],
): ConnectionStatusSample[] | null {
	let becameVisible = -1;
	for (let i = samples.length - 1; i >= 0; i--) {
		if (samples[i].onVisibilityChange && samples[i].visibility === 'visible') {
			becameVisible = i;
			break;
		}
	}
	if (becameVisible === -1) return null;
	const visibleAt = samples[becameVisible].at;
	return samples
		.slice(becameVisible)
		.filter(sample => sample.at - visibleAt <= RESUME_PAINT_MS);
}

/** What the connection-mode dialog is expected to show; a row left out is not
 *  checked. */
export interface ExpectedConnectionModes {
	isolated?: boolean;
	internet?: ConnectionModeStatus;
	localHub?: ConnectionModeStatus;
	phoneToPhone?: ConnectionModeStatus;
}

export class ConnectionStatusIndicator extends TestHelper {
	chip = this.el(tid('connection-status'));
	dialog = this.el(tid('connection-status-dialog'));
	isolationMode = this.el(tid('connection-mode-isolation'));
	internetMode = this.el(tid('connection-mode-internet'));
	localHubMode = this.el(tid('connection-mode-local-hub'));
	phoneToPhoneMode = this.el(tid('connection-mode-phone-to-phone'));

	/** Read the chip's data-status, or null while it is not mounted. */
	status(): Promise<ConnectionStatus | null> {
		return this.agent.execute(() => window.__test.connectionStatus());
	}

	/** Start recording every status the chip renders. Returns the recording's
	 *  token, to be handed back to [`recordedStatuses`]. */
	startRecordingStatus(): Promise<string> {
		return this.agent.execute(() => window.__test.recordConnectionStatus());
	}

	/** The statuses rendered since [`startRecordingStatus`], in order, each with
	 *  the epoch-ms instant it was rendered. Throws if the webview reloaded in
	 *  between, which would otherwise look like a clean recording that simply
	 *  never saw anything. */
	async recordedStatuses(token: string): Promise<ConnectionStatusSample[]> {
		const history = await this.agent.execute(() =>
			window.__test.connectionStatusHistory(),
		);
		if (history.token !== token) {
			throw new Error(
				`connection-status recording was lost (expected token ${token}, got ${history.token}) — the webview reloaded, so the statuses rendered in between were not observed`,
			);
		}
		return history.statuses;
	}

	/** True if the dialog is mounted and visible (not the closed/fade-out state). */
	isDialogOpen(): Promise<boolean> {
		return this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) return false;
			return !el.classList.contains('opacity-0');
		}, tid('connection-status-dialog'));
	}

	async waitForStatus(
		status: ConnectionStatus,
		timeout: number,
		timeoutMsg: string,
	): Promise<void> {
		let seen: ConnectionStatus | null = null;
		try {
			await this.agent.waitUntil(
				async () => {
					seen = await this.status();
					return seen === status;
				},
				{ timeout, interval: 200 },
			);
		} catch {
			throw new Error(`${timeoutMsg} (the chip last read ${String(seen)})`);
		}
	}

	/** Wait until the chip is not showing a local hub. Which other state it
	 *  reads instead depends on the cloud connection, so this leaves it open. */
	async waitForNotLocal(timeout: number, timeoutMsg: string): Promise<void> {
		let seen: ConnectionStatus | null = null;
		try {
			await this.agent.waitUntil(
				async () => {
					seen = await this.status();
					return seen !== 'local';
				},
				{ timeout, interval: 200 },
			);
		} catch {
			throw new Error(`${timeoutMsg} (the chip last read ${String(seen)})`);
		}
	}

	/** Open the dialog and wait until its local hub row names `count` hubs,
	 *  then close it. The chip must be reading local. */
	async waitForLocalHubCount(
		count: number,
		timeout: number,
		timeoutMsg: string,
	): Promise<void> {
		await this.chip.click();
		await this.localHubMode.waitForExist();
		let seen: number | null = null;
		try {
			await this.agent.waitUntil(
				async () => {
					const text = await this.localHubMode.getText();
					seen = Number(text.match(/\d+/)?.[0]);
					return seen === count;
				},
				{ timeout, interval: 200 },
			);
		} catch {
			throw new Error(`${timeoutMsg} (the dialog last named ${String(seen)})`);
		} finally {
			await this.closeDialog();
		}
	}

	/** Open the dialog, check the rows named in `expected`, then close it. */
	async expectModes(expected: ExpectedConnectionModes): Promise<void> {
		await this.chip.click();
		await this.agent.waitUntil(() => this.isDialogOpen());
		try {
			if (expected.isolated === true) {
				await expect(this.isolationMode).toExist();
			}
			if (expected.isolated === false) {
				await expect(this.isolationMode).not.toExist();
			}
			if (expected.internet !== undefined) {
				await expect(this.internetMode).toHaveAttribute(
					'data-status',
					expected.internet,
				);
			}
			if (expected.localHub !== undefined) {
				await expect(this.localHubMode).toHaveAttribute(
					'data-status',
					expected.localHub,
				);
			}
			if (expected.phoneToPhone !== undefined) {
				await expect(this.phoneToPhoneMode).toHaveAttribute(
					'data-status',
					expected.phoneToPhone,
				);
			}
		} finally {
			await this.closeDialog();
		}
	}

	/** The dialog has no button of its own: it closes on a tap outside it. */
	private async closeDialog(): Promise<void> {
		await this.agent.execute((sel: string) => {
			const backdrop = document.querySelector(sel)?.previousElementSibling;
			if (backdrop instanceof HTMLElement) backdrop.click();
		}, tid('connection-status-dialog'));
		await this.agent.waitUntil(async () => !(await this.isDialogOpen()));
	}
}
