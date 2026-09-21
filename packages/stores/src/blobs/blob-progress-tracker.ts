/** With no byte advancing for this long, a download is shown as stalled and
 * the user is offered a tap to retry. */
export const BLOB_STALL_INTERVAL_MS = 10_000;

export interface BlobState {
	bytes: number;
	complete: boolean;
	stalled: boolean;
}

/** Folds successive snapshots of one blob into a `BlobState`, flagging a stall
 * once the byte count has not advanced for `stallMs`. Stalls are judged when a
 * snapshot is applied, so there is no timer to run or dispose. */
export class BlobProgressTracker {
	state: BlobState | undefined;
	private advancedAt: number | undefined;

	constructor(
		private stallMs: number = BLOB_STALL_INTERVAL_MS,
		private now: () => number = Date.now,
	) {}

	apply(progress: { bytes: number; complete: boolean }): BlobState {
		const at = this.now();
		const previous = this.state?.bytes ?? 0;
		if (this.advancedAt === undefined || progress.bytes > previous) {
			this.advancedAt = at;
		}
		this.state = {
			bytes: progress.bytes,
			complete: progress.complete,
			stalled: !progress.complete && at - this.advancedAt >= this.stallMs,
		};
		return this.state;
	}

	/** Clear a stall after the user re-attempted the download, giving it
	 * another `stallMs` before it counts as stalled again. */
	retry(): BlobState | undefined {
		this.advancedAt = this.now();
		if (this.state !== undefined)
			this.state = { ...this.state, stalled: false };
		return this.state;
	}
}
