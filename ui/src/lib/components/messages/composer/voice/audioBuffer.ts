import WebaudioPeaks from 'webaudio-peaks';

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
 * Render a decoded buffer down to a single mono channel at `sampleRate`.
 * Used to normalize mobile recordings to the same low-rate mono WAV the
 * desktop recorder produces, keeping voice notes well under the message size
 * limit. WebAudio downmixes multi-channel sources to mono automatically.
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
 * Reduce a decoded buffer into `bars` peak-normalized amplitude values
 * (0..=255) for the scrubber. Peaks are extracted by `webaudio-peaks`; the
 * loudest bar maps to 255 so quiet recordings still fill the waveform.
 */
export function computeWaveform(
	buffer: AudioBuffer,
	bars = WAVEFORM_BARS,
): Uint8Array {
	const samplesPerPixel = Math.max(1, Math.floor(buffer.length / bars));
	// `data[0]` holds 8-bit mono min/max pairs per pixel: [min0, max0, min1, …].
	// `cueOut` must be the sample count: it is not defaulted when passed as 0,
	// which would slice an empty range and yield a flat (all-zero) waveform.
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
