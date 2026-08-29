<script module lang="ts">
	/** How long the mobile bar takes to morph out of, and back into, the message
	 * input. The composer keeps the input hidden for exactly this long on the way
	 * out, so the two pills are never stacked. Other themes swap instantly. */
	export function morphMs(theme: string): number {
		return theme === 'ios' ? 90 : 0;
	}
</script>

<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Button, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiChevronLeft } from '@mdi/js';
	import { cubicOut } from 'svelte/easing';
	import { fade } from 'svelte/transition';
	import type { VoiceRecorder } from './voice-recorder.svelte';
	import RecordingIndicator from './RecordingIndicator.svelte';
	import VoiceDesktopBar from './VoiceDesktopBar.svelte';

	interface Props {
		voice: VoiceRecorder;
	}

	let { voice }: Props = $props();

	const theme = $derived(useTheme());
	// Matches the message input's pill so every composer surface looks alike.
	const surfaceClass = $derived(
		theme === 'ios'
			? 'border border-[var(--k-hairline-color)] bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
			: 'bg-incoming-surface',
	);

	const hold = $derived(voice.view === 'hold');
	const locked = $derived(voice.view === 'locked');

	/** The trailing slot the bar keeps free for the mic/send button — see
	 * `.has-end-button`. It is also how far the bar sits from the message
	 * input's footprint, since the two are the same width. */
	const TRAILING_SLOT_PX = 50;

	/** Slides the bar off the message input and back onto it. The bar and the
	 * input are congruent — the input's pill holds the mic that the bar sets
	 * aside — so the bar starts out covering the input exactly and only needs
	 * to travel, leaving its contents undistorted. */
	function morph(node: Element, { duration }: { duration: number }) {
		const rtl = getComputedStyle(node).direction === 'rtl';
		const offset = rtl ? -TRAILING_SLOT_PX : TRAILING_SLOT_PX;
		return {
			duration,
			easing: cubicOut,
			css: (t: number) => `transform: translateX(${offset * (1 - t)}px)`,
		};
	}

	const morphParams = $derived({ duration: morphMs(theme) });
	const contentFadeMs = $derived(Math.round(morphMs(theme) / 3));
</script>

{#if hold || locked}
	<!-- One element across both mobile views so locking swaps the contents
	     instead of replaying the grow-in transition. -->
	<div
		class="voice-bar has-end-button {surfaceClass} {hold
			? 'pointer-events-none px-2 text-[var(--k-text-color)]'
			: 'ps-3 pe-2'}"
		data-testid={hold ? 'voice-recording-overlay' : 'voice-locked-bar'}
		transition:morph={morphParams}
	>
		<!-- The recorder's contents clear out well before the pill finishes
		     collapsing, so what travels back to the input is an empty pill. -->
		<div
			class="flex flex-1 items-center gap-2"
			out:fade={{ duration: contentFadeMs }}
		>
			{#if hold}
				<RecordingIndicator elapsedMs={voice.elapsedMs} />

				<div
					class="flex flex-1 items-center justify-center gap-1 text-sm"
					style="opacity: {0.6 * (1 - voice.drag.cancelProgress)}"
				>
					<wa-icon class="rtl:-scale-x-100" src={wrapPathInSvg(mdiChevronLeft)}
					></wa-icon>
					<span>{m.voiceSlideToCancel()}</span>
				</div>
			{:else}
				<RecordingIndicator elapsedMs={voice.elapsedMs} micSize={18} />

				<div class="flex-1"></div>

				<Button
					clear
					rounded
					inline
					onClick={() => void voice.cancel()}
					colors={{ textIos: 'text-red-500', textMaterial: 'text-red-500' }}
					data-testid="voice-cancel"
					style="width: auto"
				>
					{m.cancel()}
				</Button>
			{/if}
		</div>
	</div>
{:else if voice.view === 'desktop'}
	<div class="voice-bar voice-bar-flush bg-page-surface">
		<VoiceDesktopBar {voice} />
	</div>
{/if}

<style>
	.voice-bar {
		position: absolute;
		inset-block: 0;
		inset-inline: 0;
		display: flex;
		align-items: center;
		border-radius: 22px;
		/* The composer's emoji/attach/mic buttons (Konsta `Button`) sit at z-index 10;
		   the bar must paint above them so they don't bleed through. */
		z-index: 20;
	}
	/* Leave the trailing slot free so the action button (mic / send) sits outside
	   the bordered pill, mirroring the message input's send button. */
	.voice-bar.has-end-button {
		inset-inline-end: calc(42px + 0.5rem);
	}
	/* On desktop the bar spans the full row and lays out its own inner pill plus
	   the Cancel/Send buttons, so it must not look like a pill — it just paints
	   the composer surface to hide the input row underneath. */
	.voice-bar.voice-bar-flush {
		border-radius: 0;
	}
</style>
