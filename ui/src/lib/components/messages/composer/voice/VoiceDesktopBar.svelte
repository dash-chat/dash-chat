<script lang="ts">
	import { Button, Preloader } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { type FileHandle, SeekMode, open } from '@tauri-apps/plugin-fs';
	import RecordingIndicator from './RecordingIndicator.svelte';
	import type { VoiceRecorder } from './voice-recorder.svelte';

	interface Props {
		voice: VoiceRecorder;
	}

	let { voice }: Props = $props();

	const sending = $derived(voice.phase === 'encoding');

	// Skip the canonical WAV header to reach the PCM frames.
	const WAV_HEADER_BYTES = 44;
	const SAMPLE_INTERVAL_MS = 80;
	// 16 kHz mono PCM16 is 32 kB/s, so one interval plus writer lag fits easily.
	const READ_CHUNK_BYTES = 16384;
	// Speech rarely nears full scale, so normalizing against 1.0 would flatten every bar.
	const FULL_SCALE_RMS = 0.25;

	/** One RMS level per `SAMPLE_INTERVAL_MS`, oldest first. */
	let levels = $state<number[]>([]);

	// Live input levels read from the partial WAV the recorder is still writing —
	// the plugin has no metering API and the webview is denied `getUserMedia`, so
	// the file on disk is the only source. Each tick reads only new frames.
	$effect(() => {
		const path = voice.recordingPath;
		if (!path) return;
		levels = [];
		let handle: FileHandle | undefined;
		let offset = WAV_HEADER_BYTES;
		const buffer = new Uint8Array(READ_CHUNK_BYTES);
		const sample = async () => {
			if (!handle) return;
			try {
				await handle.seek(offset, SeekMode.Start);
				const read = await handle.read(buffer);
				if (read === null || read < 2) return;
				offset += read - (read % 2);
				levels.push(rms(buffer, read));
			} catch {
				// The file is mid-write or already gone; skip this tick.
			}
		};
		// Levels are decoration; a recording that can't be tailed still records.
		const opening = open(path, { read: true })
			.then(h => (handle = h))
			.catch(() => undefined);
		const timer = setInterval(() => void sample(), SAMPLE_INTERVAL_MS);
		return () => {
			clearInterval(timer);
			void opening.then(() => handle?.close().catch(() => {}));
		};
	});

	/** RMS of little-endian PCM16, normalized to a 0..1 bar height. */
	function rms(bytes: Uint8Array, length: number): number {
		const frames = Math.floor(length / 2);
		if (frames === 0) return 0;
		const view = new DataView(bytes.buffer, bytes.byteOffset);
		let sum = 0;
		for (let i = 0; i < frames; i++) {
			const normalized = view.getInt16(i * 2, true) / 32768;
			sum += normalized * normalized;
		}
		return Math.min(1, Math.sqrt(sum / frames) / FULL_SCALE_RMS);
	}

	const CAPACITY = 88;
	const MIN_HEIGHT = 8;

	// Newest at the trailing edge, older ones scrolling out of the leading one.
	const heights = $derived(
		levels
			.slice(-CAPACITY)
			.map(level => MIN_HEIGHT + level * (100 - MIN_HEIGHT)),
	);
</script>

<div class="flex w-full items-center gap-2" data-testid="voice-desktop-bar">
	<div
		class="flex min-h-[42px] min-w-0 flex-1 items-center gap-3 rounded-[22px] border border-[var(--k-hairline-color)] bg-white ps-3 pe-3 dark:bg-gray-800"
	>
		<RecordingIndicator elapsedMs={voice.elapsedMs} micSize={18} />

		<!-- Fills from the trailing edge so the first bars appear where the newest
		     audio is, rather than stretching across an empty track. -->
		<div
			class="flex h-7 min-w-0 flex-1 items-center justify-end gap-0.5 overflow-hidden"
			aria-hidden="true"
		>
			{#each heights as height, i (i)}
				<span
					class="w-0.5 flex-none rounded-full bg-current opacity-50"
					style="height: {height}%"
				></span>
			{/each}
		</div>
	</div>

	<Button
		clear
		rounded
		inline
		onClick={() => void voice.cancel()}
		data-testid="voice-cancel"
		style="width: auto"
	>
		{m.cancel()}
	</Button>

	<Button
		rounded
		inline
		onClick={() => void voice.stopAndSend()}
		disabled={sending}
		data-testid="voice-send"
		style="width: auto"
	>
		{#if sending}
			<Preloader class="h-5 w-5" />
		{:else}
			{m.send()}
		{/if}
	</Button>
</div>
