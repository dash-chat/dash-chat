<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import type { VoicePlayer } from './voice-player.svelte';

	interface Props {
		/** Peak amplitudes in 0..1, one per bar. */
		peaks: number[];
		durationSec: number;
		player: VoicePlayer;
	}

	let { peaks, durationSec, player }: Props = $props();

	let scrubbing = $state(false);

	const MIN_HEIGHT_PERCENT = 12;
	const heights = $derived(
		peaks.map(p => MIN_HEIGHT_PERCENT + p * (100 - MIN_HEIGHT_PERCENT)),
	);
	const progress = $derived(
		player.durationSec > 0
			? Math.min(1, player.currentTime / player.durationSec)
			: 0,
	);

	function seekFromPointer(clientX: number, el: HTMLElement) {
		if (durationSec <= 0) return;
		const rect = el.getBoundingClientRect();
		let fraction = (clientX - rect.left) / rect.width;
		if (getComputedStyle(el).direction === 'rtl') fraction = 1 - fraction;
		void player.seekTo(fraction);
	}

	function onPointerDown(event: PointerEvent) {
		const el = event.currentTarget as HTMLElement;
		el.setPointerCapture(event.pointerId);
		scrubbing = true;
		seekFromPointer(event.clientX, el);
	}

	function onPointerMove(event: PointerEvent) {
		const el = event.currentTarget as HTMLElement;
		if (!el.hasPointerCapture(event.pointerId)) return;
		seekFromPointer(event.clientX, el);
	}

	function onKeyDown(event: KeyboardEvent) {
		if (durationSec <= 0) return;
		const el = event.currentTarget as HTMLElement;
		// The fill grows from the inline-start, so the arrow keys follow the
		// visual fill: in RTL, leftward moves forward in time.
		const rtl = getComputedStyle(el).direction === 'rtl';
		const backKey = rtl ? 'ArrowRight' : 'ArrowLeft';
		const forwardKey = rtl ? 'ArrowLeft' : 'ArrowRight';
		if (event.key === backKey) void player.seekBy(-5);
		else if (event.key === forwardKey) void player.seekBy(5);
		else return;
		event.preventDefault();
	}
</script>

{#snippet bars(opacity: number)}
	<div
		class="flex h-full w-full items-center justify-between"
		aria-hidden="true"
	>
		{#each heights as height, i (i)}
			<span
				class="w-0.5 shrink-0 rounded-full"
				style="height: {height}%; background: currentColor; opacity: {opacity}"
			></span>
		{/each}
	</div>
{/snippet}

<div
	class="@container relative h-7 min-w-0 flex-1 cursor-pointer"
	data-testid="voice-scrubber"
	role="slider"
	tabindex="0"
	aria-label={m.voiceSeek()}
	aria-valuemin={0}
	aria-valuemax={Math.round(durationSec)}
	aria-valuenow={Math.round(player.currentTime)}
	onpointerdown={onPointerDown}
	onpointermove={onPointerMove}
	onlostpointercapture={() => (scrubbing = false)}
	onkeydown={onKeyDown}
>
	{@render bars(0.35)}
	<!-- `timeupdate` only fires ~4/sec; the linear transition smooths the bar
	     between updates. Off while paused or scrubbing so seeks snap. -->
	<div
		class="absolute inset-y-0 start-0 overflow-hidden {player.paused ||
		scrubbing
			? ''
			: 'transition-[width] duration-300 ease-linear'}"
		data-testid="voice-scrubber-played"
		style="width: {progress * 100}%"
	>
		<div class="h-full w-[100cqw]">
			{@render bars(0.95)}
		</div>
	</div>
</div>
