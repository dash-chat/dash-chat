import type { DraftVoiceNote } from '$lib/utils/media';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { mkdir, readFile, remove } from '@tauri-apps/plugin-fs';
import audioBufferToWav from 'audiobuffer-to-wav';
import {
	getDevices,
	getStatus,
	requestPermission,
	startRecording,
	stopRecording,
} from 'tauri-plugin-audio-recorder-api';

import { computeWaveform, decodeToBuffer, resampleToMono } from './audioBuffer';
import { RecordingLevels } from './recording-levels.svelte';

let warmUpPromise: Promise<unknown> | undefined;

/** Touches the cpal host up front so the first recording doesn't pay its ~2s
 * cold init. Only helps a press that follows soon after: Linux/ALSA suspends an
 * idle capture device and reopening costs ~1.9s again within seconds. */
export function warmUpRecorder(): void {
	if (warmUpPromise) return;
	warmUpPromise = getDevices().catch(() => {});
}

export type RecorderPhase =
	| 'idle'
	| 'requesting'
	| 'recording'
	| 'locked'
	| 'denied'
	| 'encoding';

const MAX_DURATION_SECONDS = 300;
const TARGET_SAMPLE_RATE = 16000;

/** Owns the voice-note recording lifecycle and is the only place that touches
 * the audio plugins. */
export class VoiceRecorder {
	phase = $state<RecorderPhase>('idle');
	elapsedMs = $state(0);
	readonly levels = new RecordingLevels();
	onMaxDuration: (() => void) | undefined;

	#timer: ReturnType<typeof setInterval> | undefined;
	#startedAt = 0;

	get isActive(): boolean {
		return this.phase === 'recording' || this.phase === 'locked';
	}

	async start(): Promise<void> {
		if (this.isActive || this.phase === 'requesting') return;
		this.phase = 'requesting';
		// The overlay is already up during `requesting`, so clear the prior take's
		// time before it can render.
		this.elapsedMs = 0;
		try {
			const permission = await requestPermission();
			if (!permission.granted) {
				this.phase = 'denied';
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
			this.elapsedMs = 0;
			this.phase = 'recording';
			this.#startTimer();
			// The plugin appends its own extension, so the file being written is not
			// the `outputPath` we asked for.
			const status = await getStatus();
			if (status.outputPath) await this.levels.start(status.outputPath);
		} catch (e) {
			this.phase = 'idle';
			throw e;
		}
	}

	lock(): void {
		if (this.phase === 'recording') this.phase = 'locked';
	}

	async stop(): Promise<DraftVoiceNote | undefined> {
		if (!this.isActive) return undefined;
		this.#stopTimer();
		await this.levels.stop();
		this.phase = 'encoding';
		// Held outside the try so a rejection from `stopRecording()` still runs the
		// `finally`; otherwise `phase` wedges on 'encoding' and the bar never leaves.
		let filePath: string | undefined;
		try {
			const result = await stopRecording();
			filePath = result.filePath;
			const recorded = await readFile(result.filePath);
			const decoded = await decodeToBuffer(recorded);
			// Desktop records WAV already; mobile records AAC/M4A and is re-encoded
			// here rather than natively, since iOS `AVAssetExportSession` can't emit WAV.
			const isWav = result.filePath.toLowerCase().endsWith('.wav');
			const buffer = isWav
				? decoded
				: await resampleToMono(decoded, TARGET_SAMPLE_RATE);
			return {
				bytes: isWav ? recorded : new Uint8Array(audioBufferToWav(buffer)),
				mimeType: 'audio/wav',
				// The recorder's wall-clock duration overshoots the decoded audio and
				// leaves the scrubber short of the end.
				durationMs: Math.round(buffer.duration * 1000),
				waveform: computeWaveform(buffer),
			};
		} finally {
			this.phase = 'idle';
			if (filePath) await cleanup([filePath]);
		}
	}

	async cancel(): Promise<void> {
		this.#stopTimer();
		await this.levels.stop();
		if (this.isActive) {
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
				this.onMaxDuration?.();
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
