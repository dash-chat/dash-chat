export const WAVEFORM_BARS = 48;

let audioContext: AudioContext | undefined;

function sharedAudioContext(): AudioContext {
	if (!audioContext) audioContext = new AudioContext();
	return audioContext;
}

export async function decodeToBuffer(bytes: Uint8Array): Promise<AudioBuffer> {
	// `decodeAudioData` detaches the passed ArrayBuffer, so hand it a copy.
	const copy = bytes.slice().buffer;
	return sharedAudioContext().decodeAudioData(copy);
}

/**
 * Reduces a decoded buffer into `bars` amplitudes (0..=255) for the scrubber,
 * with the loudest mapped to 255 so quiet recordings still fill the waveform.
 */
export function computeWaveform(
	buffer: AudioBuffer,
	bars = WAVEFORM_BARS,
): Uint8Array {
	const data = buffer.getChannelData(0);
	const bucketSize = Math.max(1, Math.floor(data.length / bars));
	const peaks = new Float32Array(bars);
	let max = 0;
	for (let i = 0; i < bars; i++) {
		peaks[i] = bucketPeak(data, i * bucketSize, bucketSize);
		if (peaks[i] > max) max = peaks[i];
	}
	const out = new Uint8Array(bars);
	if (max === 0) return out;
	for (let i = 0; i < bars; i++) {
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
