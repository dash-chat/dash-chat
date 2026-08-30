import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export type ConnectionStatus = 'connected' | 'local' | 'disconnected';

export interface ConnectionStatusSample {
	status: string;
	at: number;
	visibility: 'visible' | 'hidden';
	onVisibilityChange: boolean;
}

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

export class ConnectionStatusIndicator extends TestHelper {
	chip = this.el(tid('connection-status'));
	dialog = this.el(tid('connection-status-dialog'));
	dialogTitle = this.el(tid('connection-status-dialog-title'));
	dialogDescription = this.el(tid('connection-status-dialog-description'));
	dialogCloseButton = this.el(tid('connection-status-dialog-close'));

	/** Read the chip's data-status. Absence === 'connected'. */
	async status(): Promise<ConnectionStatus> {
		return this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) return 'connected';
			const status = el.dataset.status;
			if (status === 'local' || status === 'disconnected') return status;
			throw new Error(`connectionStatus: unexpected data-status="${status}"`);
		}, tid('connection-status'));
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
}
