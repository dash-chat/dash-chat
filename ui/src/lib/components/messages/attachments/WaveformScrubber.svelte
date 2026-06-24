<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';

	interface Props {
		bars: number[];
		progress: number;
		durationSec: number;
		currentTime: number;
		/** Seek to an absolute time (seconds), from a pointer tap/drag. */
		onseek: (timeSec: number) => void;
		/** Seek relative to the current time (seconds), from keyboard arrows. */
		onseekBy: (deltaSec: number) => void;
	}

	let { bars, progress, durationSec, currentTime, onseek, onseekBy }: Props =
		$props();

	function seekTo(clientX: number, container: HTMLElement) {
		if (durationSec <= 0) return;
		const rect = container.getBoundingClientRect();
		let fraction = (clientX - rect.left) / rect.width;
		if (getComputedStyle(container).direction === 'rtl')
			fraction = 1 - fraction;
		onseek(Math.max(0, Math.min(1, fraction)) * durationSec);
	}

	function onPointerDown(event: PointerEvent) {
		const container = event.currentTarget as HTMLElement;
		container.setPointerCapture(event.pointerId);
		seekTo(event.clientX, container);
	}

	function onPointerMove(event: PointerEvent) {
		const container = event.currentTarget as HTMLElement;
		if (!container.hasPointerCapture(event.pointerId)) return;
		seekTo(event.clientX, container);
	}

	function onKeyDown(event: KeyboardEvent) {
		if (durationSec <= 0) return;
		if (event.key === 'ArrowLeft') {
			onseekBy(-5);
		} else if (event.key === 'ArrowRight') {
			onseekBy(5);
		} else {
			return;
		}
		event.preventDefault();
	}
</script>

<div
	class="waveform flex h-7 min-w-0 flex-1 cursor-pointer items-center gap-px"
	data-testid="voice-scrubber"
	role="slider"
	tabindex="0"
	aria-label={m.voiceSeek()}
	aria-valuemin={0}
	aria-valuemax={Math.round(durationSec)}
	aria-valuenow={Math.round(currentTime)}
	onpointerdown={onPointerDown}
	onpointermove={onPointerMove}
	onkeydown={onKeyDown}
>
	{#each bars as bar, i (i)}
		<span
			class="min-h-[2px] flex-1 rounded-full"
			style="height: {Math.max(
				6,
				(bar / 255) * 100,
			)}%; background: currentColor; opacity: {i / bars.length < progress
				? 0.9
				: 0.35}"
		></span>
	{/each}
</div>

<style>
	.waveform :global(span) {
		transition: opacity 0.1s linear;
	}
</style>
