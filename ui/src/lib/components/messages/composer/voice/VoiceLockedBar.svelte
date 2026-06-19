<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSend, mdiMicrophone, mdiTrashCanOutline } from '@mdi/js';
	import { formatDuration } from '$lib/utils/time';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		/** Live elapsed time while recording hands-free. */
		elapsedMs: number;
		/** Discard the recording. */
		onCancel: () => void;
		/** Stop the recording and send it immediately. */
		onSend: () => void;
	}

	let { elapsedMs, onCancel, onSend }: Props = $props();
</script>

<div
	class="voice-locked-bar flex w-full items-center gap-2 px-1"
	data-testid="voice-locked-bar"
>
	<IconButton
		icon={mdiTrashCanOutline}
		onClick={onCancel}
		label={m.voiceCancel()}
		testid="voice-cancel"
		class="h-10 w-10 shrink-0"
	/>

	<div class="flex flex-1 items-center gap-2">
		<wa-icon class="rec-mic" src={wrapPathInSvg(mdiMicrophone)}></wa-icon>
		<span class="font-mono text-sm tabular-nums"
			>{formatDuration(elapsedMs)}</span
		>
	</div>

	<button
		type="button"
		class="send-button flex h-[42px] w-[42px] shrink-0 items-center justify-center p-0"
		data-testid="voice-send"
		aria-label={m.send()}
		onclick={onSend}
	>
		<wa-icon style="font-size: 24px" src={wrapPathInSvg(mdiSend)}></wa-icon>
	</button>
</div>

<style>
	.rec-mic {
		width: 18px;
		height: 18px;
		color: #ef4444;
		animation: pulse 1.2s ease-in-out infinite;
	}
	.send-button {
		border: none;
		border-radius: 50%;
		cursor: pointer;
		background: var(--k-theme-color, #3b82f6);
		color: white;
		transition:
			filter 0.2s ease,
			transform 0.1s ease;
	}
	.send-button:hover {
		filter: brightness(1.1);
	}
	.send-button:active {
		transform: scale(0.95);
	}
	.send-button :global(wa-icon) {
		width: 22px;
		height: 22px;
		margin-inline-start: 2px;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.3;
		}
	}
</style>
