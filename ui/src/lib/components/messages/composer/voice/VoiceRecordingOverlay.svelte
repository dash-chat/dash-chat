<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiChevronLeft } from '@mdi/js';
	import RecordingIndicator from './RecordingIndicator.svelte';
	import type { DragState } from './VoiceRecordButton.svelte';

	interface Props {
		elapsedMs: number;
		drag: DragState;
	}

	let { elapsedMs, drag }: Props = $props();
</script>

<div
	class="voice-overlay flex w-full items-center gap-2 px-2"
	data-testid="voice-recording-overlay"
>
	<RecordingIndicator {elapsedMs} timerTestid="voice-recording-timer" />

	<div
		class="slide-hint flex flex-1 items-center justify-center gap-1 text-sm opacity-60"
		style="opacity: {0.6 * (1 - drag.cancelProgress)}"
	>
		<wa-icon class="chevron" src={wrapPathInSvg(mdiChevronLeft)}></wa-icon>
		<span>{m.voiceSlideToCancel()}</span>
	</div>
</div>

<style>
	.voice-overlay {
		background: inherit;
		color: var(--k-text-color);
	}
	.chevron:dir(rtl) {
		transform: scaleX(-1);
	}
</style>
