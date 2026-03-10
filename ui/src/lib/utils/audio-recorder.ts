/**
 * Audio recorder that captures from the microphone and encodes directly to MP3.
 *
 * Uses Web Audio API (AudioContext + AudioWorkletNode) to get raw PCM,
 * then encodes to MP3 with lamejs. This avoids MediaRecorder format issues
 * (WebKitGTK records webm/opus but can't play or decode it back).
 */

import { Mp3Encoder } from '@breezystack/lamejs';

export interface AudioRecording {
	/** Base64 data URL (audio/mp3) */
	dataUrl: string;
	mimeType: string;
	/** Duration in milliseconds */
	durationMs: number;
	/** Size in bytes */
	size: number;
}

export interface AudioRecorderHandle {
	/** Stop recording and get the result. */
	stop(): Promise<AudioRecording>;
	/** Cancel recording without returning data. */
	cancel(): void;
	/** Timestamp when recording started. */
	readonly startTime: number;
}

const WORKLET_CODE = `
class PCMForwarder extends AudioWorkletProcessor {
	process(inputs) {
		const channel = inputs[0]?.[0];
		if (channel?.length > 0) {
			this.port.postMessage(channel);
		}
		return true;
	}
}
registerProcessor('pcm-forwarder', PCMForwarder);
`;

let workletRegistered: WeakSet<BaseAudioContext> = new WeakSet();

/**
 * Start recording audio from the microphone.
 * Encodes to MP3 in real-time using lamejs.
 * Returns a handle to stop/cancel the recording.
 */
export async function startRecording(): Promise<AudioRecorderHandle> {
	const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
	const audioCtx = new AudioContext();
	const source = audioCtx.createMediaStreamSource(stream);
	const sampleRate = audioCtx.sampleRate;

	const mp3Encoder = new Mp3Encoder(1, sampleRate, 128);
	const mp3Parts: Uint8Array[] = [];
	const startTime = Date.now();

	if (!workletRegistered.has(audioCtx)) {
		const blob = new Blob([WORKLET_CODE], { type: 'application/javascript' });
		const url = URL.createObjectURL(blob);
		await audioCtx.audioWorklet.addModule(url);
		URL.revokeObjectURL(url);
		workletRegistered.add(audioCtx);
	}

	const workletNode = new AudioWorkletNode(audioCtx, 'pcm-forwarder');

	workletNode.port.onmessage = (e: MessageEvent<Float32Array>) => {
		const input = e.data;
		const samples = new Int16Array(input.length);
		for (let i = 0; i < input.length; i++) {
			const s = Math.max(-1, Math.min(1, input[i]));
			samples[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
		}
		const mp3buf = mp3Encoder.encodeBuffer(samples);
		if (mp3buf.length > 0) {
			mp3Parts.push(mp3buf);
		}
	};

	source.connect(workletNode);
	workletNode.connect(audioCtx.destination);

	function cleanup() {
		workletNode.port.onmessage = null;
		workletNode.disconnect();
		source.disconnect();
		audioCtx.close();
		stream.getTracks().forEach((t) => t.stop());
	}

	return {
		get startTime() {
			return startTime;
		},

		async stop(): Promise<AudioRecording> {
			const durationMs = Date.now() - startTime;

			const end = mp3Encoder.flush();
			if (end.length > 0) {
				mp3Parts.push(end);
			}

			cleanup();

			const mp3Blob = new Blob(mp3Parts as BlobPart[], { type: 'audio/mp3' });
			const dataUrl = await blobToDataUrl(mp3Blob);

			return {
				dataUrl,
				mimeType: 'audio/mp3',
				durationMs,
				size: mp3Blob.size,
			};
		},

		cancel() {
			cleanup();
		},
	};
}

function blobToDataUrl(blob: Blob): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => resolve(reader.result as string);
		reader.onerror = reject;
		reader.readAsDataURL(blob);
	});
}

/** Format milliseconds as m:ss */
export function formatDuration(ms: number): string {
	const totalSeconds = Math.floor(ms / 1000);
	const minutes = Math.floor(totalSeconds / 60);
	const seconds = totalSeconds % 60;
	return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}
