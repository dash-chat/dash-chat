import { m } from '$lib/paraglide/messages.js';
import { isMobile } from '$lib/utils/environment';
import type { DraftVoiceNote } from '$lib/utils/media';
import { showToast } from '$lib/utils/toasts';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { mkdir, readFile, remove } from '@tauri-apps/plugin-fs';
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

// What each platform's recorder writes: desktop WAV, Android M4A, iOS ADTS AAC.
const MIME_BY_EXTENSION: Record<string, string> = {
	wav: 'audio/wav',
	m4a: 'audio/mp4',
	aac: 'audio/aac',
};

function mimeTypeFromPath(path: string): string {
	const extension = path.slice(path.lastIndexOf('.') + 1).toLowerCase();
	return MIME_BY_EXTENSION[extension] ?? 'application/octet-stream';
}

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
	#willCancel = false;
	// The pointer can be released while the native start is still in flight, so
	// the up/cancel handlers await this before acting on the recorder.
	#starting: Promise<void> | undefined;

	constructor(private onRecorded: (draft: DraftVoiceNote) => void) {}

	/** Which composer surface the recording UI occupies right now. Mobile holds
	 * show from `requesting` so they appear on press, not after startup; a
	 * locked take keeps its surface while the recording encodes. */
	get view(): 'idle' | 'hold' | 'locked' | 'desktop' {
		const active =
			this.phase === 'recording' ||
			(this.phase === 'encoding' && this.locked);
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
				await cleanup([result.filePath]);
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
		this.#willCancel = false;
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
		this.#willCancel = inlineTowardStart >= CANCEL_THRESHOLD;
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
		this.drag = idle;
		await this.#starting;
		if (this.phase !== 'recording' || this.locked) return;
		if (this.#willCancel || this.elapsedMs < MIN_DURATION_MS) {
			await this.cancel();
			if (!this.#willCancel) showToast(m.voiceRecordHint(), 'default');
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
		let granted: boolean;
		try {
			granted = await this.#start();
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
		if (handsFree && this.phase === 'recording') this.locked = true;
	}

	/** Starts a recording. Resolves `false` when the mic permission was denied,
	 * `true` otherwise (including when a recording is already active). */
	async #start(): Promise<boolean> {
		if (this.phase === 'recording' || this.phase === 'requesting') return true;
		this.phase = 'requesting';
		// The overlay is already up during `requesting`, so clear the prior take's
		// time before it can render.
		this.elapsedMs = 0;
		try {
			const permission = await requestPermission();
			if (!permission.granted) {
				this.phase = 'idle';
				return false;
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
			this.elapsedMs = 0;
			this.phase = 'recording';
			this.#startTimer();
			// The plugin appends its own extension, so the file being written is not
			// the `outputPath` we asked for.
			const status = await getStatus();
			this.recordingPath = status.outputPath ?? undefined;
			return true;
		} catch (e) {
			this.phase = 'idle';
			throw e;
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
			const recorded = await readFile(result.filePath);
			const decoded = await decodeToBuffer(recorded);
			return {
				bytes: recorded,
				mimeType: mimeTypeFromPath(result.filePath),
				// The recorder's wall-clock duration overshoots the decoded audio and
				// leaves the scrubber short of the end.
				durationMs: Math.round(decoded.duration * 1000),
				waveform: computeWaveform(decoded),
			};
		} finally {
			this.phase = 'idle';
			if (filePath) await cleanup([filePath]);
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

async function cleanup(paths: string[]): Promise<void> {
	for (const path of new Set(paths)) {
		try {
			await remove(path);
		} catch {
			// Best-effort temp cleanup.
		}
	}
}

const WAVEFORM_BARS = 48;

let audioContext: AudioContext | undefined;

function sharedAudioContext(): AudioContext {
	if (!audioContext) audioContext = new AudioContext();
	return audioContext;
}

async function decodeToBuffer(bytes: Uint8Array): Promise<AudioBuffer> {
	// `decodeAudioData` detaches the passed ArrayBuffer, so hand it a copy.
	const copy = bytes.slice().buffer;
	return sharedAudioContext().decodeAudioData(copy);
}

/**
 * Reduces a decoded buffer into `WAVEFORM_BARS` amplitudes (0..=255) for the
 * scrubber, with the loudest mapped to 255 so quiet recordings still fill the
 * waveform.
 */
function computeWaveform(buffer: AudioBuffer): Uint8Array {
	const data = buffer.getChannelData(0);
	const bucketSize = Math.max(1, Math.floor(data.length / WAVEFORM_BARS));
	const peaks = new Float32Array(WAVEFORM_BARS);
	let max = 0;
	for (let i = 0; i < WAVEFORM_BARS; i++) {
		peaks[i] = bucketPeak(data, i * bucketSize, bucketSize);
		if (peaks[i] > max) max = peaks[i];
	}
	const out = new Uint8Array(WAVEFORM_BARS);
	if (max === 0) return out;
	for (let i = 0; i < WAVEFORM_BARS; i++) {
		out[i] = Math.round((peaks[i] / max) * 255);
	}
	return out;
}

function bucketPeak(data: Float32Array, start: number, size: number): number {
	const end = Math.min(start + size, data.length);
	let peak = 0;
	for (let i = start; i < end; i++) {
		const amp = Math.abs(data[i]);
		if (amp > peak) peak = amp;
	}
	return peak;
}
