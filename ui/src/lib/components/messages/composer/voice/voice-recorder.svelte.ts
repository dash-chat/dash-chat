import { m } from '$lib/paraglide/messages.js';
import { isMobile } from '$lib/utils/environment';
import type { DraftVoiceNote } from '$lib/utils/media';
import { showToast } from '$lib/utils/toasts';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { mkdir, remove } from '@tauri-apps/plugin-fs';
import { invokeAfterSetup } from 'dash-chat-stores';
import {
	getDevices,
	getStatus,
	requestPermission,
	startRecording,
	stopRecording,
} from 'tauri-plugin-audio-recorder-api';

let warmUpPromise: Promise<unknown> | undefined;

/** Touches the cpal host up front so the first recording doesn't pay its ~2s
 * cold init (desktop-only; mobile recorders don't use cpal). Only helps a
 * press that follows soon after: Linux/ALSA suspends an idle capture device
 * and reopening costs ~1.9s again within seconds. */
export function warmUpRecorder(): void {
	if (isMobile || warmUpPromise) return;
	warmUpPromise = getDevices().catch(() => {});
}

type RecorderPhase = 'idle' | 'requesting' | 'recording' | 'encoding';

const MAX_DURATION_SECONDS = 300;

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

/** Owns the voice-note recording lifecycle — the press-and-hold gesture, the
 * recorder plugin calls, and which composer surface the recording UI shows. */
export class VoiceRecorder {
	phase = $state<RecorderPhase>('idle');
	elapsedMs = $state(0);
	drag: DragState = $state(idle);
	// A hold-and-release also passes through `encoding`, but must not surface the
	// locked bar while the WAV encodes — only a genuinely locked take should.
	locked = $state(false);
	/** Path of the file the recorder is writing, for live level metering. */
	recordingPath = $state<string | undefined>();

	#timer: ReturnType<typeof setInterval> | undefined;
	#startedAt = 0;
	#startX = 0;
	#startY = 0;
	#isRtl = false;
	// The pointer can be released while the native start is still in flight, so
	// the up/cancel handlers await this before acting on the recorder.
	#starting: Promise<void> | undefined;

	constructor(private onRecorded: (draft: DraftVoiceNote) => void) {}

	/** Which composer surface the recording UI occupies right now. Mobile holds
	 * show from `requesting` so they appear on press, not after startup; a
	 * locked take keeps its surface while the recording encodes. */
	get view(): 'idle' | 'hold' | 'locked' | 'desktop' {
		const active =
			this.phase === 'recording' || (this.phase === 'encoding' && this.locked);
		if (!isMobile) return active ? 'desktop' : 'idle';
		if (
			this.phase === 'requesting' ||
			(this.phase === 'recording' && !this.locked)
		)
			return 'hold';
		return active ? 'locked' : 'idle';
	}

	async stopAndSend(): Promise<boolean> {
		let draft: DraftVoiceNote | undefined;
		try {
			draft = await this.#stop();
		} catch (e) {
			console.error('Failed to finish voice recording', e);
			showToast(m.voiceRecordFailed(), 'error');
			return false;
		}
		if (draft) this.onRecorded(draft);
		return !!draft;
	}

	async cancel(): Promise<void> {
		this.#stopTimer();
		this.recordingPath = undefined;
		if (this.phase === 'recording') {
			try {
				const result = await stopRecording();
				await cleanup(result.filePath);
			} catch {
				// Not actually recording (e.g. permission was pending).
			}
		}
		this.phase = 'idle';
		this.elapsedMs = 0;
	}

	onPointerDown = (event: PointerEvent) => {
		event.preventDefault();
		const el = event.currentTarget as HTMLElement;
		el.setPointerCapture(event.pointerId);
		this.#startX = event.clientX;
		this.#startY = event.clientY;
		this.locked = false;
		this.#isRtl = getComputedStyle(el).direction === 'rtl';
		this.#starting = this.#startRecording(event.pointerType === 'mouse');
	};

	onPointerMove = (event: PointerEvent) => {
		if (this.phase !== 'recording' || this.locked) return;
		const inlineTowardStart = this.#isRtl
			? event.clientX - this.#startX
			: this.#startX - event.clientX;
		const up = this.#startY - event.clientY;
		this.drag = {
			cancelProgress: clamp01(inlineTowardStart / CANCEL_THRESHOLD),
			lockProgress: clamp01(up / LOCK_THRESHOLD),
		};
		if (up >= LOCK_THRESHOLD) {
			this.locked = true;
			this.drag = idle;
		}
	};

	onPointerUp = async () => {
		const willCancel = this.drag.cancelProgress >= 1;
		this.drag = idle;
		await this.#starting;
		if (this.phase !== 'recording' || this.locked) return;
		if (willCancel || this.elapsedMs < MIN_DURATION_MS) {
			await this.cancel();
			if (!willCancel) showToast(m.voiceRecordHint(), 'default');
			return;
		}
		await this.stopAndSend();
	};

	onPointerCancel = async () => {
		this.drag = idle;
		await this.#starting;
		// A locked take is hands-free, so a stray pointercancel must not end it.
		if (this.phase === 'recording' && !this.locked) await this.cancel();
	};

	async #startRecording(handsFree: boolean): Promise<void> {
		if (this.phase === 'recording' || this.phase === 'requesting') return;
		this.phase = 'requesting';
		// The overlay is already up during `requesting`, so clear the prior take's
		// time before it can render.
		this.elapsedMs = 0;
		try {
			const permission = await requestPermission();
			if (!permission.granted) {
				this.phase = 'idle';
				showToast(m.voiceMicDenied(), 'error');
				return;
			}
			// Straight into the cache dir, no subdirectory: that keeps the path
			// within the granted `scope-appcache`.
			const cache = await appCacheDir();
			await mkdir(cache, { recursive: true });
			const outputPath = await join(
				cache,
				`dc-voice-${crypto.randomUUID()}.wav`,
			);
			if (warmUpPromise) await warmUpPromise;
			await this.#discardOrphanedRecording();
			await startRecording({
				outputPath,
				format: 'wav',
				quality: 'low',
				maxDuration: MAX_DURATION_SECONDS,
			});
			this.#startedAt = Date.now();
			this.phase = 'recording';
			this.#startTimer();
			if (!isMobile) {
				// The plugin appends its own extension, so the file being written is
				// not the `outputPath` we asked for. Mobile never shows levels, so it
				// skips the lookup.
				const status = await getStatus();
				this.recordingPath = status.outputPath ?? undefined;
			}
			// A mouse can't comfortably press-and-hold, so a click records hands-free.
			if (handsFree) this.locked = true;
		} catch (e) {
			this.phase = 'idle';
			console.error('Failed to start voice recording', e);
			showToast(m.voiceRecordFailed(), 'error');
		}
	}

	async #stop(): Promise<DraftVoiceNote | undefined> {
		if (this.phase !== 'recording') return undefined;
		this.#stopTimer();
		this.recordingPath = undefined;
		this.phase = 'encoding';
		// Held outside the try so a rejection from `stopRecording()` still runs the
		// `finally`; otherwise `phase` wedges on 'encoding' and the bar never leaves.
		let filePath: string | undefined;
		try {
			const result = await stopRecording();
			filePath = result.filePath;
			// The native recorder writes a per-platform format; the backend
			// transcodes it to Ogg/Opus and derives the duration and waveform.
			const encoded = await invokeAfterSetup<{
				opus: number[];
				durationMs: number;
				waveform: number[];
			}>('transcode_voice_message', { path: result.filePath });
			return {
				bytes: new Uint8Array(encoded.opus),
				mimeType: 'audio/ogg',
				durationMs: encoded.durationMs,
				waveform: new Uint8Array(encoded.waveform),
			};
		} finally {
			this.phase = 'idle';
			if (filePath) await cleanup(filePath);
		}
	}

	// A webview reload tears down our JS state without firing onDestroy, leaving
	// the native recorder running and the next start hitting "Already recording".
	async #discardOrphanedRecording(): Promise<void> {
		try {
			const status = await getStatus();
			if (status.state !== 'idle') await stopRecording();
		} catch {
			// startRecording will surface the error.
		}
	}

	#startTimer(): void {
		this.#timer = setInterval(() => {
			this.elapsedMs = Date.now() - this.#startedAt;
			if (this.elapsedMs >= MAX_DURATION_SECONDS * 1000) {
				this.#stopTimer();
				void this.stopAndSend();
			}
		}, 100);
	}

	#stopTimer(): void {
		if (this.#timer) {
			clearInterval(this.#timer);
			this.#timer = undefined;
		}
	}
}

async function cleanup(path: string): Promise<void> {
	try {
		await remove(path);
	} catch {
		// Best-effort temp cleanup.
	}
}
