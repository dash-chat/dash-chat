import type { Hash } from '../p2panda/types';

/** With no byte advancing for this long, a download is shown as stalled and
 * the user is offered a tap to retry. */
export const BLOB_STALL_INTERVAL_MS = 10_000;

/** Payload of the `blob://progress` event and `get_blob_progress` command. */
export interface BlobProgress {
	hash: Hash;
	bytes: number;
	complete: boolean;
}

export interface BlobState {
	bytes: number;
	complete: boolean;
	stalled: boolean;
}

/** Folds progress snapshots and events for one blob into a `BlobState`,
 * flagging a stall when the byte count stops advancing for `stallMs`. */
export class BlobProgressTracker {
	state: BlobState = { bytes: 0, complete: false, stalled: false };
	#timer: ReturnType<typeof setTimeout> | undefined;
	#observed = false;

	constructor(
		private onChange: (state: BlobState) => void,
		private stallMs: number = BLOB_STALL_INTERVAL_MS,
	) {}

	apply(progress: { bytes: number; complete: boolean }): void {
		if (this.state.complete) return;
		if (progress.complete) {
			this.#clearTimer();
			this.#set({
				bytes: Math.max(this.state.bytes, progress.bytes),
				complete: true,
				stalled: false,
			});
			return;
		}
		const advanced = progress.bytes > this.state.bytes;
		if (advanced || this.#timer === undefined) this.#restartTimer();
		if (advanced || !this.#observed) {
			this.#set({
				bytes: Math.max(this.state.bytes, progress.bytes),
				complete: false,
				stalled: false,
			});
		}
	}

	retry(): void {
		if (this.state.complete) return;
		this.#restartTimer();
		this.#set({ ...this.state, stalled: false });
	}

	dispose(): void {
		this.#clearTimer();
	}

	#restartTimer(): void {
		this.#clearTimer();
		this.#timer = setTimeout(() => {
			this.#timer = undefined;
			this.#set({ ...this.state, stalled: true });
		}, this.stallMs);
	}

	#clearTimer(): void {
		if (this.#timer !== undefined) clearTimeout(this.#timer);
		this.#timer = undefined;
	}

	// The first observation always notifies, so a listener can tell "seen at
	// 0 bytes" apart from "not observed yet".
	#set(next: BlobState): void {
		const s = this.state;
		if (
			this.#observed &&
			s.bytes === next.bytes &&
			s.complete === next.complete &&
			s.stalled === next.stalled
		)
			return;
		this.#observed = true;
		this.state = next;
		this.onChange(next);
	}
}
