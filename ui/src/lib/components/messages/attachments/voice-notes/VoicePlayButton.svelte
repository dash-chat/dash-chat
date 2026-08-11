<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Preloader } from 'konsta/svelte';

	interface Props {
		paused: boolean;
		loading?: boolean;
		onclick: () => void;
	}

	let { paused, loading = false, onclick }: Props = $props();
</script>

<button
	type="button"
	class="flex h-9 w-9 shrink-0 cursor-pointer items-center justify-center rounded-full border-none text-inherit"
	style="background: color-mix(in srgb, currentColor 15%, transparent)"
	data-testid="voice-play-button"
	aria-label={paused ? m.voicePlay() : m.voicePause()}
	aria-busy={loading}
	{onclick}
>
	{#if loading}
		<Preloader class="h-[18px] w-[18px]" />
	{:else if paused}
		<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
			<path d="M8 5v14l11-7z" />
		</svg>
	{:else}
		<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
			<path d="M6 5h4v14H6zM14 5h4v14h-4z" />
		</svg>
	{/if}
</button>
