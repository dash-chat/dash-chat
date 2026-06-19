<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { VoiceNote } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { objectUrl } from '$lib/actions/object-url';
	import { formatDuration } from '$lib/utils/time';

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

	function toggle() {
		if (!audioEl) return;
		if (audioEl.paused) audioEl.play();
		else audioEl.pause();
	}

	function seekTo(clientX: number, container: HTMLElement) {
		if (!audioEl || durationSec <= 0) return;
		const rect = container.getBoundingClientRect();
		let fraction = (clientX - rect.left) / rect.width;
		if (getComputedStyle(container).direction === 'rtl')
			fraction = 1 - fraction;
		audioEl.currentTime = Math.max(0, Math.min(1, fraction)) * durationSec;
	}

	function onScrubPointerDown(event: PointerEvent) {
		const container = event.currentTarget as HTMLElement;
		container.setPointerCapture(event.pointerId);
		seekTo(event.clientX, container);
	}

	function onScrubPointerMove(event: PointerEvent) {
		const container = event.currentTarget as HTMLElement;
		if (!container.hasPointerCapture(event.pointerId)) return;
		seekTo(event.clientX, container);
	}

	function onScrubKeyDown(event: KeyboardEvent) {
		if (!audioEl || durationSec <= 0) return;
		if (event.key === 'ArrowLeft') {
			audioEl.currentTime = Math.max(0, audioEl.currentTime - 5);
		} else if (event.key === 'ArrowRight') {
			audioEl.currentTime = Math.min(durationSec, audioEl.currentTime + 5);
		} else {
			return;
		}
		event.preventDefault();
	}
</script>

<div
	class="flex items-center gap-3 px-1 py-0.5"
	style="width: 240px; max-width: 100%"
	data-testid="message-attachment-voice"
>
	<audio
		bind:this={audioEl}
		bind:paused
		bind:currentTime
		onloadedmetadata={() => {
			if (audioEl && isFinite(audioEl.duration))
				loadedDuration = audioEl.duration;
		}}
		onended={() => audioEl && (audioEl.currentTime = 0)}
		use:objectUrl={{ data: voice.data, mimeType: voice.mime_type }}
	></audio>

	<button
		type="button"
		class="flex h-9 w-9 shrink-0 cursor-pointer items-center justify-center rounded-full border-none text-inherit"
		style="background: color-mix(in srgb, currentColor 15%, transparent)"
		data-testid="voice-play-button"
		aria-label={paused ? m.voicePlay() : m.voicePause()}
		onclick={toggle}
	>
		{#if paused}
			<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
				<path d="M8 5v14l11-7z" />
			</svg>
		{:else}
			<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
				<path d="M6 5h4v14H6zM14 5h4v14h-4z" />
			</svg>
		{/if}
	</button>

	<div class="flex min-w-0 flex-1 flex-col gap-1">
		<div
			class="waveform flex h-7 cursor-pointer items-center gap-px"
			data-testid="voice-scrubber"
			role="slider"
			tabindex="0"
			aria-label={m.voiceSeek()}
			aria-valuemin={0}
			aria-valuemax={Math.round(durationSec)}
			aria-valuenow={Math.round(currentTime)}
			onpointerdown={onScrubPointerDown}
			onpointermove={onScrubPointerMove}
			onkeydown={onScrubKeyDown}
		>
			{#each bars as bar, i (i)}
				<span
					class="min-h-[2px] flex-1 rounded-full"
					style="height: {Math.max(
						6,
						(bar / 255) * 100,
					)}%; background: currentColor; opacity: {i / bars.length < progress
						? 0.9
						: 0.35}"
				></span>
			{/each}
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
</div>

<style>
	.waveform :global(span) {
		transition: opacity 0.1s linear;
	}
</style>
