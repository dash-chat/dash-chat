<script lang="ts">
	import { tick, type Snippet } from 'svelte';
	import type { VoiceNote } from 'dash-chat-stores';
	import { objectUrl } from '$lib/actions/object-url';
	import { blobUrl } from '$lib/utils/media';
	import { formatDuration } from '$lib/utils/time';
	import VoicePlayButton from './VoicePlayButton.svelte';
	import WaveformScrubber from './WaveformScrubber.svelte';

	interface Props {
		voice: VoiceNote;
		/** Timestamp / receipts rendered inline at the end of the row, Signal-style
		 * (only on a voice-only message; a captioned note shows them below). */
		metadata?: Snippet;
	}

	let { voice, metadata }: Props = $props();

	const bars = $derived(Array.from(voice.waveform));

	let audioEl: HTMLAudioElement | undefined = $state();
	let paused = $state(true);
	let currentTime = $state(0);
	let loadedDuration = $state(0);

	// `voice.duration_ms` is authoritative; the decoded WAV's reported duration
	// is only a fallback for the scrubber once metadata loads.
	const durationSec = $derived(
		loadedDuration > 0 ? loadedDuration : voice.duration_ms / 1000,
	);
	const progress = $derived(
		durationSec > 0 ? Math.min(1, currentTime / durationSec) : 0,
	);
	const labelMs = $derived(
		paused && currentTime === 0 ? voice.duration_ms : currentTime * 1000,
	);

	// WebKitGTK's <audio> can't load our custom `irohblob://` scheme (its media
	// pipeline bypasses the webview's scheme handler), so fetch the bytes and
	// play them from a blob: URL built by the `objectUrl` action (which revokes
	// it on destroy). Fetched lazily on first play.
	let audioSource = $state<{ data: Uint8Array; mimeType: string }>();
	let loadPromise: Promise<boolean> | undefined;

	function ensureLoaded(): Promise<boolean> {
		if (audioSource) return Promise.resolve(true);
		if (loadPromise) return loadPromise;
		loadPromise = (async () => {
			try {
				const res = await fetch(blobUrl(voice.hash));
				if (!res.ok) return false;
				audioSource = {
					data: new Uint8Array(await res.arrayBuffer()),
					mimeType: voice.mime_type,
				};
				await tick(); // let the `objectUrl` action set <audio>.src
				return true;
			} catch {
				return false;
			} finally {
				loadPromise = undefined;
			}
		})();
		return loadPromise;
	}

	async function toggle() {
		if (!audioEl) return;
		if (!audioEl.paused) {
			audioEl.pause();
			return;
		}
		if (await ensureLoaded()) audioEl.play();
	}

	function seek(timeSec: number) {
		if (audioEl) audioEl.currentTime = timeSec;
	}

	function seekBy(deltaSec: number) {
		if (!audioEl) return;
		audioEl.currentTime = Math.max(
			0,
			Math.min(durationSec, audioEl.currentTime + deltaSec),
		);
	}
</script>

<div
	class="flex flex-col gap-1 px-1 py-0.5"
	style="width: 240px; max-width: 100%"
	data-testid="message-attachment-voice"
>
	<audio
		bind:this={audioEl}
		bind:paused
		bind:currentTime
		use:objectUrl={audioSource}
		onloadedmetadata={() => {
			if (audioEl && isFinite(audioEl.duration))
				loadedDuration = audioEl.duration;
		}}
		onended={() => audioEl && (audioEl.currentTime = 0)}
	></audio>

	<div class="flex items-center gap-3">
		<VoicePlayButton {paused} onclick={toggle} />

		<WaveformScrubber
			{bars}
			{progress}
			{durationSec}
			{currentTime}
			onseek={seek}
			onseekBy={seekBy}
		/>
	</div>

	<div class="flex items-center justify-between text-xs opacity-70">
		<span>{formatDuration(labelMs)}</span>
		{#if metadata}
			<span class="flex items-center gap-1 whitespace-nowrap select-none">
				{@render metadata()}
			</span>
		{/if}
	</div>
</div>
