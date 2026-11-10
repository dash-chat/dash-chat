import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export class ConnectionStatusBar extends TestHelper {
	openButton = this.el(tid('connection-status'));
	collapseButton = this.el(tid('connection-status-bar-collapse'));
	bar = this.el(tid('connection-status-bar'));
	internetChannel = this.el(tid('connection-status-bar-internet'));
	hubChannel = this.el(tid('connection-status-bar-hub'));
	p2pChannel = this.el(tid('connection-status-bar-p2p'));
	dialog = this.el(tid('connection-status-dialog'));
	dialogTitle = this.el(tid('connection-status-dialog-title'));
	dialogDescription = this.el(tid('connection-status-dialog-description'));
	dialogCloseButton = this.el(tid('connection-status-dialog-close'));

	/** True if the dialog is mounted and visible (not the closed/fade-out state). */
	isDialogOpen(): Promise<boolean> {
		return this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) return false;
			return !el.classList.contains('opacity-0');
		}, tid('connection-status-dialog'));
	}

	/** Open the dialog and wait until its text says `count` local hubs are
	 *  connected, then close it. The status must be reading local. */
	async waitForLocalHubCount(
		count: number,
		timeout: number,
		timeoutMsg: string,
	): Promise<void> {
		await this.openDialog();
		await this.dialogDescription.waitForExist();
		let seen: number | null = null;
		try {
			await this.agent.waitUntil(
				async () => {
					const text = await this.dialogDescription.getText();
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

	/** Open the dialog, check it reads `title` and `description`, then close it. */
	async expectDialog(title: string, description: string): Promise<void> {
		await this.openDialog();
		await this.agent.waitUntil(() => this.isDialogOpen());
		try {
			await expect(this.dialogTitle).toHaveText(title);
			await expect(this.dialogDescription).toHaveText(description);
		} finally {
			await this.closeDialog();
		}
	}

	private async openDialog(): Promise<void> {
		await this.openButton.click();
		await this.bar.click();
	}

	private async closeDialog(): Promise<void> {
		await this.dialogCloseButton.click();
		await this.agent.waitUntil(async () => !(await this.isDialogOpen()));
		await this.collapseButton.click();
		await this.bar.waitForExist({ reverse: true });
	}
}
