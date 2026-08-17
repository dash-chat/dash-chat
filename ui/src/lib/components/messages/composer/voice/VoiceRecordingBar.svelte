<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Button, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiChevronLeft } from '@mdi/js';
	import type { VoiceControl } from './voice-control.svelte';
	import RecordingIndicator from './RecordingIndicator.svelte';
	import VoiceDesktopBar from './VoiceDesktopBar.svelte';

	interface Props {
		voice: VoiceControl;
	}

	let { voice }: Props = $props();

	const theme = $derived(useTheme());
	const surfaceClass = $derived(
		theme === 'ios'
			? 'bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
			: 'bg-white dark:bg-gray-800',
	);
</script>

{#if voice.view === 'hold'}
	<div
		class="voice-bar has-end-button pointer-events-none gap-2 px-2 text-[var(--k-text-color)] {surfaceClass}"
		data-testid="voice-recording-overlay"
	>
		<RecordingIndicator elapsedMs={voice.recorder.elapsedMs} />

		<div
			class="flex flex-1 items-center justify-center gap-1 text-sm"
			style="opacity: {0.6 * (1 - voice.drag.cancelProgress)}"
		>
			<wa-icon class="chevron" src={wrapPathInSvg(mdiChevronLeft)}></wa-icon>
			<span>{m.voiceSlideToCancel()}</span>
		</div>
	</div>
{:else if voice.view === 'locked'}
	<div
		class="voice-bar has-end-button gap-2 ps-3 pe-2 {surfaceClass}"
		data-testid="voice-locked-bar"
	>
		<RecordingIndicator elapsedMs={voice.recorder.elapsedMs} micSize={18} />

		<div class="flex-1"></div>

		<Button
			clear
			rounded
			inline
			onClick={() => void voice.cancel()}
			colors={{ textIos: 'text-red-500', textMaterial: 'text-red-500' }}
			aria-label={m.voiceCancel()}
			data-testid="voice-cancel"
			style="width: auto"
		>
			{m.cancel()}
		</Button>
	</div>
{:else if voice.view === 'desktop'}
	<div class="voice-bar voice-bar-flush bg-page-surface">
		<VoiceDesktopBar
			elapsedMs={voice.recorder.elapsedMs}
			levels={voice.recorder.levels.levels}
			onCancel={() => void voice.cancel()}
			onSend={() => voice.stopAndSend()}
		/>
	</div>
{/if}

<style>
	.voice-bar {
		position: absolute;
		inset-block: 0;
		inset-inline: 0;
		display: flex;
		align-items: center;
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
		/* The composer's emoji/attach/mic buttons (Konsta `Button`) sit at z-index 10;
		   the bar must paint above them so they don't bleed through. */
		z-index: 20;
	}
	/* Leave the trailing slot free so the action button (mic / send) sits outside
	   the bordered pill, mirroring the message input's send button. */
	.voice-bar.has-end-button {
		inset-inline-end: calc(42px + 0.5rem);
	}
	/* On desktop the bar spans the full row and lays out its own inner pill plus
	   the Cancel/Send buttons, so it must not look like a pill — it just paints
	   the composer surface to hide the input row underneath. */
	.voice-bar.voice-bar-flush {
		border: none;
		border-radius: 0;
	}
	.chevron:dir(rtl) {
		transform: scaleX(-1);
	}
</style>
