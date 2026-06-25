<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { VoiceNote } from 'dash-chat-stores';
	import { formatDuration } from '$lib/utils/time';
	import { AudioSourceLoader, type LoadedAudio } from './useAudioSource.svelte';
	import VoicePlayButton from './VoicePlayButton.svelte';
	import Waveform from './Waveform.svelte';

	interface Props {
		voice: VoiceNote;
		/** Timestamp / receipts rendered inline at the end of the row, Signal-style
		 * (only on a voice-only message; a captioned note shows them below). */
		metadata?: Snippet;
	}

	let { voice, metadata }: Props = $props();

	const peaks = $derived(Array.from(voice.waveform, v => v / 255));
	const durationSec = $derived(voice.duration_ms / 1000);

	let paused = $state(true);
	let currentTime = $state(0);
	let waveform: ReturnType<typeof Waveform> | undefined = $state();

	// `voice.duration_ms` is authoritative; while playing we show the elapsed time.
	const labelMs = $derived(
		paused && currentTime === 0 ? voice.duration_ms : currentTime * 1000,
	);

	const audio = new AudioSourceLoader(() => voice);

	async function loadAudio(): Promise<LoadedAudio | undefined> {
		return (await audio.ensureLoaded()) ? audio.source : undefined;
	}
</script>

<div
	class="flex flex-col gap-1 px-1 py-0.5"
	style="width: 240px; max-width: 100%"
	data-testid="message-attachment-voice"
>
	<div class="flex items-center gap-3">
		<VoicePlayButton {paused} onclick={() => void waveform?.toggle()} />

		<Waveform
			bind:this={waveform}
			{peaks}
			{durationSec}
			{loadAudio}
			bind:paused
			bind:currentTime
		/>
	</div>

	<div class="flex items-center justify-between text-xs opacity-70">
		<span class="w-9 shrink-0 text-center">{formatDuration(labelMs)}</span>
		{#if metadata}
			<span class="flex items-center gap-1 whitespace-nowrap select-none">
				{@render metadata()}
			</span>
		{/if}
	</div>
</div>
