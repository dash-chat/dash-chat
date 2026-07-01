<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import WaveSurfer from 'wavesurfer.js';
	import { m } from '$lib/paraglide/messages.js';
	import type { VoicePlayer } from './voice-player.svelte';

	interface Props {
		/** Peak amplitudes in 0..1, one per bar. */
		peaks: number[];
		durationSec: number;
		/** Shared playback state/logic; the scrubber renders its progress and
		 * drives seeking through it. */
		player: VoicePlayer;
	}

	let { peaks, durationSec, player }: Props = $props();

	let container: HTMLDivElement;
	let wavesurfer: WaveSurfer | undefined;

	function parseRgb(color: string): [number, number, number] {
		const [r = '0', g = '0', b = '0'] = color.match(/\d+(\.\d+)?/g) ?? [];
		return [Number(r), Number(g), Number(b)];
	}

	// wavesurfer builds the progress (played) canvas by compositing `progressColor`
	// onto the wave canvas with `source-in`, which multiplies the alphas — so a
	// translucent `waveColor` caps the played region's alpha at the unplayed value
	// and the progress is invisible. Bake opaque colors against the bubble
	// background instead: the unplayed bars look identical to a translucent fill,
	// while the played bars stay fully opaque and clearly visible.
	function mixOver(color: string, background: string, alpha: number): string {
		const [r, g, b] = parseRgb(color);
		const [br, bg, bb] = parseRgb(background);
		const mix = (f: number, b: number) =>
			Math.round(f * alpha + b * (1 - alpha));
		return `rgb(${mix(r, br)}, ${mix(g, bg)}, ${mix(b, bb)})`;
	}

	/** First ancestor background that isn't fully transparent — the surface the
	 * waveform is painted on, used to flatten the bars to opaque colors. */
	function resolveBackground(el: HTMLElement): string {
		let node: HTMLElement | null = el;
		while (node) {
			const bg = getComputedStyle(node).backgroundColor;
			const parts = bg.match(/\d+(\.\d+)?/g);
			if (parts && (parts.length < 4 || Number(parts[3]) > 0)) return bg;
			node = node.parentElement;
		}
		return 'rgb(0, 0, 0)';
	}

	// wavesurfer renders only the static bars; playback runs through the shared
	// player's own `<audio>` (wavesurfer's media element doesn't advance
	// `currentTime` for blob audio on iOS WKWebView). Drive the played region from
	// the player's clock against its real duration.
	function renderProgress() {
		const duration = player.durationSec;
		if (!wavesurfer || duration <= 0) return;
		wavesurfer
			.getRenderer()
			.renderProgress(
				Math.min(1, player.currentTime / duration),
				!player.paused,
			);
	}

	$effect(() => {
		// Re-render whenever the shared clock or play state changes.
		player.currentTime;
		player.paused;
		renderProgress();
	});

	onMount(() => {
		const color = getComputedStyle(container).color;
		const background = resolveBackground(container);
		wavesurfer = WaveSurfer.create({
			container,
			height: 28,
			barWidth: 2,
			barGap: 2,
			barRadius: 2,
			cursorWidth: 0,
			interact: false,
			normalize: false,
			waveColor: mixOver(color, background, 0.35),
			progressColor: mixOver(color, background, 0.95),
			peaks: [peaks],
			duration: durationSec,
		});
	});

	onDestroy(() => {
		wavesurfer?.destroy();
	});

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
		// The scrubber is visually mirrored in RTL (`rtl:-scale-x-100`), and pointer
		// seeking flips for it too, so the arrow keys must follow the visual fill:
		// in RTL the leftward arrow moves forward in time and vice versa.
		const rtl = getComputedStyle(el).direction === 'rtl';
		const backKey = rtl ? 'ArrowRight' : 'ArrowLeft';
		const forwardKey = rtl ? 'ArrowLeft' : 'ArrowRight';
		if (event.key === backKey) void player.seekBy(-5);
		else if (event.key === forwardKey) void player.seekBy(5);
		else return;
		event.preventDefault();
	}
</script>

<div
	bind:this={container}
	class="h-7 min-w-0 flex-1 cursor-pointer rtl:-scale-x-100"
	data-testid="voice-scrubber"
	role="slider"
	tabindex="0"
	aria-label={m.voiceSeek()}
	aria-valuemin={0}
	aria-valuemax={Math.round(durationSec)}
	aria-valuenow={Math.round(player.currentTime)}
	onpointerdown={onPointerDown}
	onpointermove={onPointerMove}
	onkeydown={onKeyDown}
></div>
