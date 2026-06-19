/** Number of amplitude bars stored for the voice-note scrubber UI. */
export const WAVEFORM_BARS = 48;

let audioContext: AudioContext | undefined;

function sharedAudioContext(): AudioContext {
	if (!audioContext) audioContext = new AudioContext();
	return audioContext;
}

/** Decode an audio file's bytes into a PCM `AudioBuffer`. */
export async function decodeToBuffer(bytes: Uint8Array): Promise<AudioBuffer> {
	// `decodeAudioData` detaches the passed ArrayBuffer, so hand it a copy.
	const copy = bytes.slice().buffer;
	return sharedAudioContext().decodeAudioData(copy);
}

/**
 * Downsample a decoded buffer into `bars` peak-normalized amplitude values
 * (0..=255) for the scrubber. Uses RMS per bucket of the first channel.
 */
export function computeWaveform(
	buffer: AudioBuffer,
	bars = WAVEFORM_BARS,
): Uint8Array {
	const samples = buffer.getChannelData(0);
	const bucketSize = Math.max(1, Math.floor(samples.length / bars));
	const rms = new Float32Array(bars);
	let peak = 0;
	for (let i = 0; i < bars; i++) {
		const start = i * bucketSize;
		const end = Math.min(start + bucketSize, samples.length);
		let sum = 0;
		for (let j = start; j < end; j++) sum += samples[j] * samples[j];
		const value = end > start ? Math.sqrt(sum / (end - start)) : 0;
		rms[i] = value;
		if (value > peak) peak = value;
	}
	const out = new Uint8Array(bars);
	for (let i = 0; i < bars; i++) {
		out[i] = peak > 0 ? Math.round((rms[i] / peak) * 255) : 0;
	}
	return out;
}
