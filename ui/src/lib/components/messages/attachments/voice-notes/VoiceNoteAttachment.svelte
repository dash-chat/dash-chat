<script lang="ts">
	import { getContext, untrack, type Snippet } from 'svelte';
	import type { BlobStore, VoiceNote } from 'dash-chat-stores';
	import { formatDuration } from '$lib/utils/time';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { onScreen } from '$lib/utils/on-screen';
	import { VoicePlayer } from './voice-player.svelte';
	import VoicePlayButton from './VoicePlayButton.svelte';
	import Waveform from './Waveform.svelte';

	interface Props {
		voice: VoiceNote;
		/** Inline at the end of the row; a captioned note shows them below instead. */
		metadata?: Snippet;
	}

	let { voice, metadata }: Props = $props();

	const blobStore: BlobStore = getContext('blob-store');
	let visible = $state(false);
	const hash = $derived(voice.hash);
	const progress = $derived(
		visible ? useReactiveValue(blobStore.progress, hash) : undefined,
	);

	const peaks = $derived(Array.from(voice.waveform, v => v / 255));

	// A tap on a note still downloading asks the node for it now and says so;
	// playback waits for a tap once the blob has landed.
	function onPlayClick() {
		if ($progress !== undefined && !$progress.complete) {
			blobStore.retry(hash);
			showToast(m.fileStillDownloading());
			return;
		}
		void player.toggle();
	}

	const player = untrack(
		() => new VoicePlayer(voice, () => showToast(m.voicePlayFailed(), 'error')),
	);

	const labelMs = $derived(
		player.paused && player.currentTime === 0
			? voice.duration_ms
			: player.currentTime * 1000,
	);
</script>

<div
	class="flex w-60 max-w-full flex-col gap-1 px-1 py-0.5"
	data-testid="message-attachment-voice"
	{@attach onScreen(v => (visible = v))}
>
	<audio {@attach (el: HTMLAudioElement) => player.attach(el)}></audio>

	<div class="flex items-center gap-3">
		<VoicePlayButton
			paused={player.paused}
			loading={player.loading}
			onclick={onPlayClick}
			download={$progress}
			totalBytes={voice.size}
		/>

		<Waveform {peaks} {player} />
	</div>

	<div class="flex items-center justify-between text-xs">
		<span
			class="w-9 shrink-0 text-center opacity-70"
			data-testid="voice-duration">{formatDuration(labelMs)}</span
		>
		{#if metadata}
			<span class="flex items-center gap-1 whitespace-nowrap select-none">
				{@render metadata()}
			</span>
		{/if}
	</div>
</div>
