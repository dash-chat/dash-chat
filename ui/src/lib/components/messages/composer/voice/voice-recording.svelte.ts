import { m } from '$lib/paraglide/messages.js';
import { isMobile } from '$lib/utils/environment';
import type { DraftVoiceNote } from '$lib/utils/media';
import { showToast } from '$lib/utils/toasts';

import { VoiceRecorder, warmUpRecorder } from './useVoiceRecorder.svelte';

export interface DragState {
	cancelProgress: number;
	lockProgress: number;
}

/** Pixels the pointer must travel toward the inline-start to cancel. */
const CANCEL_THRESHOLD = 120;
/** Pixels the pointer must travel upward to lock hands-free recording. */
const LOCK_THRESHOLD = 80;
/** A press shorter than this is treated as a tap, not a recording. */
const MIN_DURATION_MS = 600;

const idle: DragState = { cancelProgress: 0, lockProgress: 0 };

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
	// The pointer can be released while the native start is still in flight, so
	// the up/cancel handlers await this before acting on the recorder.
	private starting: Promise<void> | undefined;

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

	onPointerDown = (event: PointerEvent) => {
		event.preventDefault();
		const el = event.currentTarget as HTMLElement;
		el.setPointerCapture(event.pointerId);
		this.startX = event.clientX;
		this.startY = event.clientY;
		this.willCancel = false;
		this.wasLocked = false;
		this.isRtl = getComputedStyle(el).direction === 'rtl';
		this.starting = this.startRecording(event.pointerType === 'mouse');
	};

	onPointerMove = (event: PointerEvent) => {
		if (this.recorder.phase !== 'recording') return;
		const inlineTowardStart = this.isRtl
			? event.clientX - this.startX
			: this.startX - event.clientX;
		const up = this.startY - event.clientY;
		this.willCancel = inlineTowardStart >= CANCEL_THRESHOLD;
		this.drag = {
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
		await this.starting;
		if (this.recorder.phase !== 'recording') return;
		if (this.willCancel || this.recorder.elapsedMs < MIN_DURATION_MS) {
			await this.recorder.cancel();
			if (!this.willCancel) showToast(m.voiceRecordHint(), 'default');
			return;
		}
		await this.stopAndSend();
	};

	onPointerCancel = async () => {
		this.drag = idle;
		await this.starting;
		// A locked take is hands-free, so a stray pointercancel must not end it.
		if (this.recorder.phase === 'recording') await this.recorder.cancel();
	};

	private async startRecording(handsFree: boolean): Promise<void> {
		let granted: boolean;
		try {
			granted = await this.recorder.start();
		} catch (e) {
			console.error('Failed to start voice recording', e);
			showToast(m.voiceRecordFailed(), 'error');
			return;
		}
		if (!granted) {
			showToast(m.voiceMicDenied(), 'error');
			return;
		}
		// A mouse can't comfortably press-and-hold, so a click records hands-free.
		if (handsFree && this.recorder.phase === 'recording') {
			this.recorder.lock();
			this.wasLocked = true;
		}
	}
}
