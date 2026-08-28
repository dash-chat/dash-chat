<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Preloader } from 'konsta/svelte';
	import { mdiPause, mdiPlay } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';

	interface Props {
		paused: boolean;
		loading: boolean;
		onclick: () => void;
	}

	let { paused, loading, onclick }: Props = $props();
</script>

<Button
	clear
	rounded
	inline
	onClick={onclick}
	class="!h-9 !w-9 shrink-0 !p-0 !text-inherit"
	style="background: color-mix(in srgb, currentColor 15%, transparent)"
	data-testid="voice-play-button"
	aria-label={paused ? m.voicePlay() : m.voicePause()}
	aria-busy={loading}
>
	{#if loading}
		<Preloader class="h-[18px] w-[18px]" />
	{:else}
		<wa-icon class="text-lg" src={wrapPathInSvg(paused ? mdiPlay : mdiPause)}
		></wa-icon>
	{/if}
</Button>
