<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import WaveSurfer from 'wavesurfer.js';
	import { m } from '$lib/paraglide/messages.js';
	import type { LoadedAudio } from './useAudioSource.svelte';

	interface Props {
		/** Peak amplitudes in 0..1, one per bar. */
		peaks: number[];
		durationSec: number;
		/** Lazily resolve the audio bytes on first play; undefined on failure. */
		loadAudio: () => Promise<LoadedAudio | undefined>;
		paused?: boolean;
		currentTime?: number;
	}

	let {
		peaks,
		durationSec,
		loadAudio,
		paused = $bindable(true),
		currentTime = $bindable(0),
	}: Props = $props();

	let container: HTMLDivElement;
	let audio: HTMLAudioElement | undefined;
	let wavesurfer: WaveSurfer | undefined;
	let objectUrl: string | undefined;
	let loaded = false;
	let rafId: number | undefined;

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

	// The real playback length: prefer the loaded `<audio>` element's own duration
	// over the recorded `durationSec` metadata, which can overshoot and leave the
	// played region short of the end. Falls back to the metadata before the audio
	// loads or when the platform reports a non-finite duration.
	function playbackDuration(): number {
		const d = audio?.duration;
		return d && isFinite(d) && d > 0 ? d : durationSec;
	}

	// wavesurfer renders only the static bars; playback runs through a plain
	// `<audio>`. We never load audio into wavesurfer because its media element
	// doesn't advance `currentTime` for blob audio on iOS WKWebView, so its
	// built-in progress/duration are unreliable. Instead we drive the played
	// region from the `<audio>` clock against its real duration.
	function drawProgress() {
		const duration = playbackDuration();
		if (!audio || !wavesurfer || duration <= 0) return;
		currentTime = audio.currentTime;
		wavesurfer
			.getRenderer()
			.renderProgress(Math.min(1, audio.currentTime / duration), !audio.paused);
	}

	// The `<audio>` element's `timeupdate` event is throttled (~4/sec), so the
	// played region would visibly lag. Repaint each frame while playing instead,
	// matching wavesurfer's own 60fps progress animation.
	function tick() {
		drawProgress();
		if (audio && !audio.paused) rafId = requestAnimationFrame(tick);
		else rafId = undefined;
	}

	function startTicking() {
		if (rafId === undefined) rafId = requestAnimationFrame(tick);
	}

	function stopTicking() {
		if (rafId !== undefined) {
			cancelAnimationFrame(rafId);
			rafId = undefined;
		}
	}

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
		stopTicking();
		wavesurfer?.destroy();
		if (objectUrl) URL.revokeObjectURL(objectUrl);
	});

	// WebKitGTK's media pipeline can't load our custom blob scheme, so the bytes
	// are fetched lazily on first play and set as an object URL.
	async function ensureLoaded(): Promise<boolean> {
		if (loaded) return true;
		const source = await loadAudio();
		if (!audio || !source) return false;
		objectUrl = URL.createObjectURL(
			new Blob([source.data], { type: source.mimeType }),
		);
		audio.src = objectUrl;
		loaded = true;
		return true;
	}

	export async function toggle(): Promise<void> {
		if (!audio) return;
		if (!audio.paused) {
			audio.pause();
			return;
		}
		if (await ensureLoaded()) await audio.play();
	}

	async function seekTo(fraction: number) {
		if (!(await ensureLoaded()) || !audio) return;
		audio.currentTime = Math.max(0, Math.min(1, fraction)) * playbackDuration();
		drawProgress();
	}

	function seekFromPointer(clientX: number, el: HTMLElement) {
		if (durationSec <= 0) return;
		const rect = el.getBoundingClientRect();
		let fraction = (clientX - rect.left) / rect.width;
		if (getComputedStyle(el).direction === 'rtl') fraction = 1 - fraction;
		void seekTo(fraction);
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

	async function seekBy(deltaSec: number) {
		if (!(await ensureLoaded()) || !audio) return;
		audio.currentTime = Math.max(
			0,
			Math.min(playbackDuration(), audio.currentTime + deltaSec),
		);
		drawProgress();
	}

	function onKeyDown(event: KeyboardEvent) {
		if (durationSec <= 0) return;
		if (event.key === 'ArrowLeft') void seekBy(-5);
		else if (event.key === 'ArrowRight') void seekBy(5);
		else return;
		event.preventDefault();
	}
</script>

<audio
	bind:this={audio}
	onplay={() => {
		paused = false;
		startTicking();
	}}
	onpause={() => {
		paused = true;
		stopTicking();
		drawProgress();
	}}
	ontimeupdate={drawProgress}
	onended={() => {
		paused = true;
		stopTicking();
		if (audio) audio.currentTime = 0;
		drawProgress();
	}}
></audio>

<div
	bind:this={container}
	class="h-7 min-w-0 flex-1 cursor-pointer rtl:-scale-x-100"
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
></div>
