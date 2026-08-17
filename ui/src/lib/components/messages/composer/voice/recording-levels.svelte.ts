import { type FileHandle, SeekMode, open } from '@tauri-apps/plugin-fs';

// Skip the canonical WAV header to reach the PCM frames.
const WAV_HEADER_BYTES = 44;
const SAMPLE_INTERVAL_MS = 80;
// 16 kHz mono PCM16 is 32 kB/s, so one interval plus writer lag fits easily.
const READ_CHUNK_BYTES = 16384;
// Speech rarely nears full scale, so normalizing against 1.0 would flatten every bar.
const FULL_SCALE_RMS = 0.25;

/** Live input levels read from the partial WAV the recorder is still writing —
 * the plugin has no metering API and the webview is denied `getUserMedia`, so
 * the file on disk is the only source. Each tick reads only new frames. */
export class RecordingLevels {
	/** One RMS level per `SAMPLE_INTERVAL_MS`, oldest first. */
	levels: number[] = $state([]);

	#handle: FileHandle | undefined;
	#timer: ReturnType<typeof setInterval> | undefined;
	#offset = WAV_HEADER_BYTES;
	#buffer = new Uint8Array(READ_CHUNK_BYTES);

	async start(path: string): Promise<void> {
		await this.stop();
		this.levels = [];
		this.#offset = WAV_HEADER_BYTES;
		try {
			this.#handle = await open(path, { read: true });
		} catch {
			// Levels are decoration; a recording that can’t be tailed still records.
			return;
		}
		this.#timer = setInterval(() => void this.#sample(), SAMPLE_INTERVAL_MS);
	}

	async stop(): Promise<void> {
		if (this.#timer) {
			clearInterval(this.#timer);
			this.#timer = undefined;
		}
		const handle = this.#handle;
		this.#handle = undefined;
		if (handle) await handle.close().catch(() => {});
		this.levels = [];
	}

	async #sample(): Promise<void> {
		const handle = this.#handle;
		if (!handle) return;
		try {
			await handle.seek(this.#offset, SeekMode.Start);
			const read = await handle.read(this.#buffer);
			if (read === null || read < 2) return;
			this.#offset += read - (read % 2);
			this.levels.push(rms(this.#buffer, read));
		} catch {
			// The file is mid-write or already gone; skip this tick.
		}
	}
}

/** RMS of little-endian PCM16, normalized to a 0..1 bar height. */
function rms(bytes: Uint8Array, length: number): number {
	const frames = Math.floor(length / 2);
	if (frames === 0) return 0;
	let sum = 0;
	for (let i = 0; i < frames; i++) {
		const lo = bytes[i * 2];
		const hi = bytes[i * 2 + 1];
		const sample = ((hi << 24) >> 16) | lo;
		const normalized = sample / 32768;
		sum += normalized * normalized;
	}
	return Math.min(1, Math.sqrt(sum / frames) / FULL_SCALE_RMS);
}
