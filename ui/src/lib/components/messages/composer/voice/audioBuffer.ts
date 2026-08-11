import WebaudioPeaks from 'webaudio-peaks';

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
 * Renders a decoded buffer down to mono at `sampleRate`, normalizing mobile
 * recordings to the low-rate WAV the desktop recorder already produces.
 */
export async function resampleToMono(
	buffer: AudioBuffer,
	sampleRate: number,
): Promise<AudioBuffer> {
	const frames = Math.max(1, Math.ceil(buffer.duration * sampleRate));
	const offline = new OfflineAudioContext(1, frames, sampleRate);
	const source = offline.createBufferSource();
	source.buffer = buffer;
	source.connect(offline.destination);
	source.start();
	return offline.startRendering();
}

/**
 * Reduces a decoded buffer into `bars` amplitudes (0..=255) for the scrubber,
 * with the loudest mapped to 255 so quiet recordings still fill the waveform.
 */
export function computeWaveform(
	buffer: AudioBuffer,
	bars = WAVEFORM_BARS,
): Uint8Array {
	const samplesPerPixel = Math.max(1, Math.floor(buffer.length / bars));
	// `data[0]` holds 8-bit min/max pairs per pixel. `cueOut` must be the sample
	// count: it is not defaulted at 0, which would yield a flat waveform.
	const { data } = WebaudioPeaks(
		buffer,
		samplesPerPixel,
		true,
		0,
		buffer.length,
		8,
	);
	const peaks = data[0];
	const pixels = Math.floor(peaks.length / 2);
	if (pixels === 0) return new Uint8Array(bars);

	const amplitudes = new Float32Array(bars);
	let max = 0;
	for (let i = 0; i < bars; i++) {
		const p = Math.min(i, pixels - 1);
		const amp = Math.max(Math.abs(peaks[p * 2]), Math.abs(peaks[p * 2 + 1]));
		amplitudes[i] = amp;
		if (amp > max) max = amp;
	}

	const out = new Uint8Array(bars);
	for (let i = 0; i < bars; i++) {
		out[i] = max > 0 ? Math.round((amplitudes[i] / max) * 255) : 0;
	}
	return out;
}
