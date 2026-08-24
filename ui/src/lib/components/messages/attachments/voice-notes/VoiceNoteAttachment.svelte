<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import type { VoiceNote } from 'dash-chat-stores';
	import { formatDuration } from '$lib/utils/time';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';
	import { VoicePlayer } from './voice-player.svelte';
	import VoicePlayButton from './VoicePlayButton.svelte';
	import Waveform from './Waveform.svelte';

	interface Props {
		voice: VoiceNote;
		/** Inline at the end of the row; a captioned note shows them below instead. */
		metadata?: Snippet;
	}

	let { voice, metadata }: Props = $props();

	const peaks = $derived(Array.from(voice.waveform, v => v / 255));

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
>
	<audio {@attach (el: HTMLAudioElement) => player.attach(el)}></audio>

	<div class="flex items-center gap-3">
		<VoicePlayButton
			paused={player.paused}
			loading={player.loading}
			onclick={() => void player.toggle()}
		/>

		<Waveform {peaks} {player} />
	</div>

	<div class="flex items-center justify-between text-xs opacity-70">
		<span class="w-9 shrink-0 text-center">{formatDuration(labelMs)}</span>
		{#if metadata}
			<span class="flex items-center gap-1 whitespace-nowrap select-none">
				{@render metadata()}
			</span>
		{/if}
	</div>
</div>
