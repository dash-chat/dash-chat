<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { BlobState } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Preloader } from 'konsta/svelte';
	import { mdiPause, mdiPlay } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import BlobProgressRing from '$lib/components/BlobProgressRing.svelte';

	interface Props {
		paused: boolean;
		loading: boolean;
		onclick: () => void;
		/** Blob download state; while incomplete the ring wraps the button and
		 * play is disabled. */
		download?: BlobState;
		/** Total blob bytes, for the ring's fill. */
		totalBytes?: number;
		onretry?: () => void;
	}

	let {
		paused,
		loading,
		onclick,
		download,
		totalBytes = 0,
		onretry,
	}: Props = $props();

	const downloading = $derived(download !== undefined && !download.complete);
	const stalled = $derived(download?.stalled === true);

	function handleClick() {
		if (downloading) {
			if (stalled) onretry?.();
			return;
		}
		onclick();
	}
</script>

<div class="relative inline-flex shrink-0">
	<Button
		clear
		rounded
		inline
		onClick={handleClick}
		class="!h-9 !w-9 !p-0 !text-inherit {downloading && !stalled
			? 'opacity-50'
			: ''}"
		style="background: color-mix(in srgb, currentColor 15%, transparent)"
		data-testid="voice-play-button"
		aria-label={stalled
			? m.blobDownloadStalledRetry()
			: paused
				? m.voicePlay()
				: m.voicePause()}
		aria-busy={loading}
		aria-disabled={downloading && !stalled}
	>
		{#if loading}
			<Preloader class="h-[18px] w-[18px]" />
		{:else if !stalled}
			<wa-icon class="text-lg" src={wrapPathInSvg(paused ? mdiPlay : mdiPause)}
			></wa-icon>
		{/if}
	</Button>
	{#if downloading && download}
		<div
			class="pointer-events-none absolute inset-0 flex items-center justify-center"
		>
			<BlobProgressRing
				bytes={download.bytes}
				total={totalBytes}
				{stalled}
				size={36}
			/>
		</div>
	{/if}
</div>
