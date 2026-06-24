<script lang="ts">
	import { Button } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import RecordingIndicator from './RecordingIndicator.svelte';

	interface Props {
		/** Live elapsed time while recording. */
		elapsedMs: number;
		/** Discard the recording. */
		onCancel: () => void;
		/** Stop the recording and send it immediately. */
		onSend: () => Promise<boolean>;
	}

	let { elapsedMs, onCancel, onSend }: Props = $props();

	let sending = $state(false);

	async function handleSend() {
		if (sending) return;
		sending = true;
		try {
			await onSend();
		} finally {
			sending = false;
		}
	}

	// A fixed, waveform-shaped set of bar heights (percent) so the recording
	// track reads as a waveform at rest; each bar also pulses while recording.
	const bars = Array.from({ length: 88 }, (_, i) => {
		const wave = Math.sin(i * 0.7) * Math.sin(i * 0.23 + 1);
		return 22 + Math.abs(wave) * 70;
	});
</script>

<div class="flex w-full items-center gap-2" data-testid="voice-desktop-bar">
	<div
		class="voice-pill flex min-h-[42px] min-w-0 flex-1 items-center gap-3 ps-3 pe-3 bg-white dark:bg-gray-800"
	>
		<RecordingIndicator
			{elapsedMs}
			micSize={18}
			timerTestid="voice-recording-timer"
		/>

		<div
			class="wave flex h-7 min-w-0 flex-1 items-center justify-between"
			aria-hidden="true"
		>
			{#each bars as height, i (i)}
				<span style="height: {height}%; animation-delay: {(i % 14) * 90}ms"
				></span>
			{/each}
		</div>
	</div>

	<Button
		clear
		rounded
		inline
		onClick={onCancel}
		data-testid="voice-cancel"
		style="width: auto"
	>
		{m.cancel()}
	</Button>

	<Button
		rounded
		inline
		onClick={handleSend}
		disabled={sending}
		data-testid="voice-send"
		style="width: auto"
	>
		{m.send()}
	</Button>
</div>

<style>
	.voice-pill {
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
	}
	.wave span {
		width: 2px;
		border-radius: 9999px;
		background: currentColor;
		opacity: 0.5;
		transform-origin: center;
		animation: wave 1.1s ease-in-out infinite;
	}
	@keyframes wave {
		0%,
		100% {
			transform: scaleY(0.5);
			opacity: 0.4;
		}
		50% {
			transform: scaleY(1);
			opacity: 0.75;
		}
	}
</style>
