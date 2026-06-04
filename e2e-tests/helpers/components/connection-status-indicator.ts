import { tid } from '../selectors';

export type ConnectionStatus = 'connected' | 'local' | 'disconnected';

export class ConnectionStatusIndicator {
	constructor(private agent: WebdriverIO.Browser) {}

	chip = this.agent.$(tid('connection-status'));
	dialog = this.agent.$(tid('connection-status-dialog'));
	dialogTitle = this.agent.$(tid('connection-status-dialog-title'));
	dialogDescription = this.agent.$(tid('connection-status-dialog-description'));
	dialogCloseButton = this.agent.$(tid('connection-status-dialog-close'));

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

	/** True if the dialog is mounted and visible (not the closed/fade-out state). */
	isDialogOpen(): Promise<boolean> {
		return this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) return false;
			return !el.classList.contains('opacity-0');
		}, tid('connection-status-dialog'));
	}
}
