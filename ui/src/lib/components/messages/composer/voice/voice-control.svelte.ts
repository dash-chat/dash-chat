import { m } from '$lib/paraglide/messages.js';
import { isMobile } from '$lib/utils/environment';
import type { DraftVoiceNote } from '$lib/utils/media';
import { showToast } from '$lib/utils/toasts';

import { VoiceRecorder } from './voice-recorder.svelte';

interface DragState {
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

/** The press-and-hold recording gesture and which composer surface it shows.
 * Lives outside both components because the mic sits inside the input pill
 * while the bars span the composer row. */
export class VoiceControl {
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

	/** Which composer surface the recording UI occupies right now. Mobile holds
	 * show from `requesting` so they appear on press, not after startup; a
	 * locked take keeps its surface while the recording encodes. */
	get view(): 'idle' | 'hold' | 'locked' | 'desktop' {
		const { phase } = this.recorder;
		const active =
			this.recorder.isActive || (phase === 'encoding' && this.wasLocked);
		if (!isMobile) return active ? 'desktop' : 'idle';
		if (phase === 'recording' || phase === 'requesting') return 'hold';
		return active ? 'locked' : 'idle';
	}

	async stopAndSend(): Promise<void> {
		let draft: DraftVoiceNote | undefined;
		try {
			draft = await this.recorder.stop();
		} catch (e) {
			console.error('Failed to finish voice recording', e);
			showToast(m.voiceRecordFailed(), 'error');
			return;
		}
		if (draft) this.onRecorded(draft);
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
