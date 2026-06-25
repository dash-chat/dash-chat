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

	/** wavesurfer paints to a canvas, so it needs concrete colors rather than
	 * `currentColor`; resolve the host text color and apply the played/unplayed
	 * opacities. */
	function withAlpha(color: string, alpha: number): string {
		const [r = '0', g = '0', b = '0'] = color.match(/\d+(\.\d+)?/g) ?? [];
		return `rgba(${r}, ${g}, ${b}, ${alpha})`;
	}

	// wavesurfer renders only the static bars; playback runs through a plain
	// `<audio>`. We never load audio into wavesurfer because its media element
	// doesn't advance `currentTime` for blob audio on iOS WKWebView, so its
	// built-in progress/duration are unreliable. Instead we drive the played
	// region from the `<audio>` clock against the authoritative `durationSec`.
	function drawProgress() {
		if (!audio || !wavesurfer || durationSec <= 0) return;
		currentTime = audio.currentTime;
		wavesurfer
			.getRenderer()
			.renderProgress(
				Math.min(1, audio.currentTime / durationSec),
				!audio.paused,
			);
	}

	onMount(() => {
		const color = getComputedStyle(container).color;
		wavesurfer = WaveSurfer.create({
			container,
			height: 28,
			barWidth: 2,
			barGap: 2,
			barRadius: 2,
			cursorWidth: 0,
			interact: false,
			normalize: false,
			waveColor: withAlpha(color, 0.35),
			progressColor: withAlpha(color, 0.9),
			peaks: [peaks],
			duration: durationSec,
		});
	});

	onDestroy(() => {
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
		audio.currentTime = Math.max(0, Math.min(1, fraction)) * durationSec;
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
			Math.min(durationSec, audio.currentTime + deltaSec),
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
	onplay={() => (paused = false)}
	onpause={() => (paused = true)}
	ontimeupdate={drawProgress}
	onended={() => {
		paused = true;
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
