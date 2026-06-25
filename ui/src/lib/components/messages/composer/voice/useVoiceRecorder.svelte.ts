import type { DraftVoiceNote } from '$lib/utils/media';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { mkdir, readFile, remove } from '@tauri-apps/plugin-fs';
import audioBufferToWav from 'audiobuffer-to-wav';
import {
	getStatus,
	requestPermission,
	startRecording,
	stopRecording,
} from 'tauri-plugin-audio-recorder-api';

import { computeWaveform, decodeToBuffer, resampleToMono } from './audioBuffer';

export type RecorderPhase =
	| 'idle'
	| 'requesting'
	| 'recording'
	| 'locked'
	| 'denied'
	| 'encoding';

/** Hard cap on recording length (seconds); keeps the WAV well under the
 * message size limit and matches Signal's generous-but-bounded behavior. */
const MAX_DURATION_SECONDS = 300;

/** Mono sample rate for normalized voice notes. Matches the desktop recorder's
 * "low" preset, and keeps a full-length recording well under the message size
 * limit when re-encoding a mobile recording to WAV. */
const TARGET_SAMPLE_RATE = 16000;

/**
 * Owns the voice-note recording lifecycle: microphone permission, the native
 * recorder plugin, the elapsed timer, and turning a finished recording into a
 * playable WAV `DraftVoiceNote` (normalizing mobile M4A → WAV and computing
 * the waveform). It is the only place that touches the audio plugins.
 */
export class VoiceRecorder {
	phase = $state<RecorderPhase>('idle');
	elapsedMs = $state(0);
	/** Invoked when the hard duration cap is reached so the caller can finish
	 * the recording the same way a manual stop would. */
	onMaxDuration: (() => void) | undefined;

	#timer: ReturnType<typeof setInterval> | undefined;
	#startedAt = 0;

	get isActive(): boolean {
		return this.phase === 'recording' || this.phase === 'locked';
	}

	async start(): Promise<void> {
		if (this.isActive || this.phase === 'requesting') return;
		this.phase = 'requesting';
		// Reset now so the optimistic overlay (shown during `requesting`, before the
		// native recorder has started) reads 0:00 instead of the prior take's time.
		this.elapsedMs = 0;
		try {
			const permission = await requestPermission();
			if (!permission.granted) {
				this.phase = 'denied';
				return;
			}
			// Write straight into the app cache dir (the recorder plugin creates
			// the file itself); ensure the dir exists first. Avoiding a
			// subdirectory keeps the path within the granted `scope-appcache`.
			const cache = await appCacheDir();
			await mkdir(cache, { recursive: true });
			const outputPath = await join(
				cache,
				`dc-voice-${crypto.randomUUID()}.wav`,
			);
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
		this.phase = 'encoding';
		const result = await stopRecording();
		try {
			const recorded = await readFile(result.filePath);
			const decoded = await decodeToBuffer(recorded);
			// Desktop already records low-rate mono WAV; mobile records AAC/M4A,
			// which we decode and re-encode to WAV so every peer can play the same
			// bytes. iOS `AVAssetExportSession` can't output WAV, so the conversion
			// is done here in the webview rather than via the native media plugin.
			const isWav = result.filePath.toLowerCase().endsWith('.wav');
			const buffer = isWav
				? decoded
				: await resampleToMono(decoded, TARGET_SAMPLE_RATE);
			return {
				bytes: isWav ? recorded : new Uint8Array(audioBufferToWav(buffer)),
				mimeType: 'audio/wav',
				durationMs: result.durationMs,
				waveform: computeWaveform(buffer),
			};
		} finally {
			this.phase = 'idle';
			await cleanup([result.filePath]);
		}
	}

	async cancel(): Promise<void> {
		this.#stopTimer();
		if (this.isActive) {
			try {
				const result = await stopRecording();
				await cleanup([result.filePath]);
			} catch {
				// Not actually recording (e.g. permission was pending) — nothing to do.
			}
		}
		this.phase = 'idle';
		this.elapsedMs = 0;
	}

	// A webview reload mid-recording tears down our JS state without firing
	// onDestroy, leaving the native recorder running; stop any such orphaned
	// session so the next start doesn't hit "Already recording".
	async #discardOrphanedRecording(): Promise<void> {
		try {
			const status = await getStatus();
			if (status.state !== 'idle') await stopRecording();
		} catch {
			// Best effort — if this fails, startRecording will surface the error.
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
