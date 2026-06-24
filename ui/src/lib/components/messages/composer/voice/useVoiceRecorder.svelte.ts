import type { DraftVoiceNote } from '$lib/utils/media';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { mkdir, readFile, remove } from '@tauri-apps/plugin-fs';
import {
	getStatus,
	requestPermission,
	startRecording,
	stopRecording,
} from 'tauri-plugin-audio-recorder-api';
import { convert } from 'tauri-plugin-media-toolkit-api';

import { computeWaveform, decodeToBuffer } from './encodeWav';

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
		const temps = [result.filePath];
		try {
			const wavPath = await ensureWav(result.filePath);
			if (wavPath !== result.filePath) temps.push(wavPath);
			const bytes = await readFile(wavPath);
			const waveform = computeWaveform(await decodeToBuffer(bytes));
			return {
				bytes,
				mimeType: 'audio/wav',
				durationMs: result.durationMs,
				waveform,
			};
		} finally {
			this.phase = 'idle';
			await cleanup(temps);
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

/** Normalize a recording to WAV. Desktop already records WAV; mobile records
 * M4A/AAC, which we convert so every peer can play the same bytes. */
async function ensureWav(filePath: string): Promise<string> {
	if (filePath.toLowerCase().endsWith('.wav')) return filePath;
	const base = filePath.replace(/\.[^.]+$/, '');
	const result = await convert({
		inputPath: filePath,
		outputPath: base,
		format: 'wav',
		audioQuality: 'low',
	});
	return result.outputPath;
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
