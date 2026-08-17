<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { onDestroy, onMount } from 'svelte';
	import { mdiMicrophone, mdiLockOutline, mdiChevronUp } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { isMobile } from '$lib/utils/environment';
	import IconButton from '$lib/components/IconButton.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';
	import type { VoiceControl } from './voice-control.svelte';

	interface Props {
		voice: VoiceControl;
	}

	let { voice }: Props = $props();

	onMount(() => voice.warmUp());

	// Free the mic if we leave the chat mid-recording.
	onDestroy(() => void voice.cancel());
</script>

{#if voice.view === 'locked'}
	<div class="relative z-30 shrink-0">
		<SendButton onSend={() => voice.stopAndSend()} testid="voice-send" />
	</div>
{:else if voice.view !== 'desktop'}
	<div class="relative shrink-0 {voice.view === 'hold' ? 'z-30' : ''}">
		{#if voice.view === 'hold'}
			<div
				class="lock-pill pointer-events-none absolute bottom-full start-1/2 mb-2 flex flex-col items-center gap-1.5 rounded-full bg-gray-100 px-1.5 py-2.5 dark:bg-gray-700"
				style="transform: translate(-50%, {-8 * voice.drag.lockProgress}px)"
			>
				<wa-icon
					class="lock-icon"
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
			loading={voice.recorder.phase === 'requesting' && !isMobile}
			iconClass={voice.view === 'hold' ? 'text-2xl text-white' : 'text-2xl'}
			class="!h-[42px] !w-[42px] shrink-0 touch-none {voice.view === 'hold'
				? '!bg-red-500 !opacity-100'
				: ''}"
			onpointerdown={voice.onPointerDown}
			onpointermove={voice.onPointerMove}
			onpointerup={voice.onPointerUp}
			onpointercancel={voice.onPointerCancel}
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
