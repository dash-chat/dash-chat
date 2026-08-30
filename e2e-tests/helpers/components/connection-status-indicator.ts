import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export type ConnectionStatus = 'connected' | 'local' | 'disconnected';

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

	/** The statuses rendered since [`startRecordingStatus`], in order. Throws if
	 *  the webview reloaded in between, which would otherwise look like a clean
	 *  recording that simply never saw anything. */
	async recordedStatuses(token: string): Promise<string[]> {
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
