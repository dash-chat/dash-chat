<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { onDestroy, onMount } from 'svelte';
	import { mdiMicrophone, mdiLockOutline, mdiChevronUp } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { isMobile } from '$lib/utils/environment';
	import IconButton from '$lib/components/IconButton.svelte';
	import { type VoiceRecorder, warmUpRecorder } from './voice-recorder.svelte';

	interface Props {
		voice: VoiceRecorder;
	}

	let { voice }: Props = $props();

	const hold = $derived(voice.view === 'hold');

	onMount(warmUpRecorder);

	// Free the mic if we leave the chat mid-recording.
	onDestroy(() => void voice.cancel());
</script>

<!-- `visible` re-shows the button when the composer hides the input row
     underneath the recording bar. While locked, the send button renders in the
     composer's trailing slot instead (where the attach button sits). -->
{#if voice.view === 'locked'}
	<!-- Holds the mic's place inside the pill: without a 42px child the hidden
	     input pill collapses by 2px, and the locked bar sizes to it. -->
	<div class="h-[42px] w-[42px] shrink-0"></div>
{:else if voice.view !== 'desktop'}
	<div class="visible relative shrink-0 {hold ? 'z-30' : ''}">
		{#if hold}
			<!-- left-1/2 + translate(-50%) is symmetric centering, so RTL doesn't apply. -->
			<div
				class="lock-pill pointer-events-none absolute bottom-full left-1/2 mb-2 flex flex-col items-center gap-1.5 rounded-full bg-gray-100 px-1.5 py-2.5 dark:bg-gray-700"
				style="transform: translate(-50%, {-8 * voice.drag.lockProgress}px)"
			>
				<wa-icon
					src={wrapPathInSvg(mdiLockOutline)}
					style="opacity: {0.55 + 0.45 * voice.drag.lockProgress}"
				></wa-icon>
				<wa-icon class="chevron-up" src={wrapPathInSvg(mdiChevronUp)}></wa-icon>
			</div>
		{/if}

		<IconButton
			icon={mdiMicrophone}
			label={m.voiceRecordHint()}
			testid="message-input-voice-record"
			loading={voice.phase === 'requesting' && !isMobile}
			iconClass={hold ? 'text-2xl text-white' : undefined}
			class="!h-[42px] !w-[42px] shrink-0 touch-none {hold
				? '!bg-red-500 !opacity-100'
				: ''}"
			onPointerDown={voice.onPointerDown}
			onPointerMove={voice.onPointerMove}
			onPointerUp={voice.onPointerUp}
			onPointerCancel={voice.onPointerCancel}
		/>
	</div>
{/if}

<style>
	.lock-pill :global(wa-icon) {
		width: 18px;
		height: 18px;
		color: var(--k-text-color);
	}
	.lock-pill .chevron-up {
		animation: nudge-up 1s ease-in-out infinite;
	}
	@keyframes nudge-up {
		0%,
		100% {
			transform: translateY(0);
			opacity: 0.5;
		}
		50% {
			transform: translateY(-3px);
			opacity: 0.9;
		}
	}
</style>
