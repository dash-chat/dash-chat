import { m } from '$lib/paraglide/messages.js';
import { isMobile } from '$lib/utils/environment';
import type { DraftVoiceNote } from '$lib/utils/media';
import { showToast } from '$lib/utils/toasts';

import { VoiceRecorder, warmUpRecorder } from './useVoiceRecorder.svelte';

export interface DragState {
	active: boolean;
	cancelProgress: number;
	lockProgress: number;
}

/** Pixels the pointer must travel toward the inline-start to cancel. */
const CANCEL_THRESHOLD = 120;
/** Pixels the pointer must travel upward to lock hands-free recording. */
const LOCK_THRESHOLD = 80;
/** A press shorter than this is treated as a tap, not a recording. */
const MIN_DURATION_MS = 600;

const idle: DragState = { active: false, cancelProgress: 0, lockProgress: 0 };

function clamp01(value: number): number {
	return Math.max(0, Math.min(1, value));
}

/** The press-and-hold recording gesture and its phase. Lives outside both
 * components because the mic sits inside the input pill while the bars span the
 * composer row. */
export class VoiceRecording {
	readonly recorder = new VoiceRecorder();
	drag: DragState = $state(idle);

	private startX = 0;
	private startY = 0;
	private isRtl = false;
	private willCancel = false;
	// A hold-and-release also passes through `encoding`, but must not surface the
	// locked bar while the WAV encodes — only a genuinely locked take should.
	private wasLocked = $state(false);
	// The pointer can be released while `recorder.start()` is still awaiting the
	// native start, which would otherwise leave it stuck recording.
	private releasedWhileStarting = false;

	constructor(private onRecorded: (draft: DraftVoiceNote) => void) {
		this.recorder.onMaxDuration = () => void this.stopAndSend();
	}

	get showLockedBar(): boolean {
		return (
			this.recorder.phase === 'locked' ||
			(this.recorder.phase === 'encoding' && this.wasLocked)
		);
	}

	// Hold visuals are touch-only (desktop click-records straight into the locked
	// bar), and show from `requesting` so they appear on press, not after startup.
	get recordingHoldMobile(): boolean {
		return (
			(this.recorder.phase === 'recording' ||
				this.recorder.phase === 'requesting') &&
			isMobile
		);
	}

	// The mic overlaps the bar’s own send button and, painting later, would steal
	// its taps.
	get barReplacesButton(): boolean {
		return (
			!this.recordingHoldMobile &&
			(this.showLockedBar || this.recorder.isActive)
		);
	}

	warmUp() {
		if (!isMobile) warmUpRecorder();
	}

	async stopAndSend(): Promise<boolean> {
		let draft: DraftVoiceNote | undefined;
		try {
			draft = await this.recorder.stop();
		} catch (e) {
			console.error('Failed to finish voice recording', e);
			showToast(m.voiceRecordFailed(), 'error');
			return false;
		}
		if (draft) this.onRecorded(draft);
		return !!draft;
	}

	cancel(): Promise<void> {
		return this.recorder.cancel();
	}

	onPointerDown = async (event: PointerEvent) => {
		event.preventDefault();
		const el = event.currentTarget as HTMLElement;
		el.setPointerCapture(event.pointerId);
		this.startX = event.clientX;
		this.startY = event.clientY;
		this.willCancel = false;
		this.releasedWhileStarting = false;
		this.wasLocked = false;
		this.isRtl = getComputedStyle(el).direction === 'rtl';
		await this.recorder.start();
		if (this.recorder.phase === 'denied') {
			showToast(m.voiceMicDenied(), 'error');
			this.recorder.phase = 'idle';
			return;
		}
		if (this.recorder.phase !== 'recording') return;
		// A mouse can't comfortably press-and-hold, so a click records hands-free.
		if (event.pointerType === 'mouse') {
			this.recorder.lock();
			this.wasLocked = true;
			return;
		}
		if (this.releasedWhileStarting) await this.finishHold();
	};

	onPointerMove = (event: PointerEvent) => {
		if (this.recorder.phase !== 'recording') return;
		const inlineTowardStart = this.isRtl
			? event.clientX - this.startX
			: this.startX - event.clientX;
		const up = this.startY - event.clientY;
		this.willCancel = inlineTowardStart >= CANCEL_THRESHOLD;
		this.drag = {
			active: true,
			cancelProgress: clamp01(inlineTowardStart / CANCEL_THRESHOLD),
			lockProgress: clamp01(up / LOCK_THRESHOLD),
		};
		if (up >= LOCK_THRESHOLD) {
			this.recorder.lock();
			this.wasLocked = true;
			this.drag = idle;
		}
	};

	onPointerUp = async () => {
		this.drag = idle;
		// Released before the async start finished: defer the finish to onPointerDown.
		if (this.recorder.phase === 'requesting') {
			this.releasedWhileStarting = true;
			return;
		}
		if (this.recorder.phase !== 'recording') return;
		await this.finishHold();
	};

	onPointerCancel = async () => {
		this.drag = idle;
		if (this.recorder.phase === 'requesting') {
			this.willCancel = true;
			this.releasedWhileStarting = true;
			return;
		}
		await this.recorder.cancel();
	};

	private async finishHold() {
		if (this.willCancel || this.recorder.elapsedMs < MIN_DURATION_MS) {
			await this.recorder.cancel();
			if (!this.willCancel) showToast(m.voiceRecordHint(), 'default');
			return;
		}
		await this.stopAndSend();
	}
}
