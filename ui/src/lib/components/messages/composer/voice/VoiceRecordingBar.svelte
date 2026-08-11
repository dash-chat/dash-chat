<script lang="ts">
	import { useTheme } from 'konsta/svelte';
	import { isMobile } from '$lib/utils/environment';
	import type { VoiceRecording } from './voice-recording.svelte';
	import VoiceRecordingOverlay from './VoiceRecordingOverlay.svelte';
	import VoiceLockedBar from './VoiceLockedBar.svelte';
	import VoiceDesktopBar from './VoiceDesktopBar.svelte';

	interface Props {
		voice: VoiceRecording;
		/** Trailing buttons the bar leaves uncovered so they stay reachable. */
		endButtons: 1 | 2;
	}

	let { voice, endButtons }: Props = $props();

	const theme = $derived(useTheme());
	const surfaceClass = $derived(
		theme === 'ios'
			? 'bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
			: 'bg-white dark:bg-gray-800',
	);
</script>

{#if voice.recordingHoldMobile}
	<div
		class="voice-bar pointer-events-none {surfaceClass}"
		class:has-end-button={endButtons === 1}
		class:has-two-end-buttons={endButtons === 2}
	>
		<VoiceRecordingOverlay
			elapsedMs={voice.recorder.elapsedMs}
			drag={voice.drag}
		/>
	</div>
{:else if voice.showLockedBar || voice.recorder.isActive}
	{#if isMobile}
		<div
			class="voice-bar {surfaceClass}"
			class:has-end-button={endButtons === 1}
			class:has-two-end-buttons={endButtons === 2}
		>
			<VoiceLockedBar
				elapsedMs={voice.recorder.elapsedMs}
				onCancel={() => void voice.cancel()}
			/>
		</div>
	{:else}
		<div class="voice-bar voice-bar-flush bg-page-surface">
			<VoiceDesktopBar
				elapsedMs={voice.recorder.elapsedMs}
				levels={voice.recorder.levels.levels}
				onCancel={() => void voice.cancel()}
				onSend={() => voice.stopAndSend()}
			/>
		</div>
	{/if}
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
	/* Desktop keeps the attach button alongside the mic in that trailing area. */
	.voice-bar.has-two-end-buttons {
		inset-inline-end: calc(2 * 42px + 1rem);
	}
	/* On desktop the bar spans the full row and lays out its own inner pill plus
	   the Cancel/Send buttons, so it must not look like a pill — it just paints
	   the composer surface to hide the input row underneath. */
	.voice-bar.voice-bar-flush {
		border: none;
		border-radius: 0;
	}
</style>
