<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import RecordingIndicator from './RecordingIndicator.svelte';

	interface Props {
		elapsedMs: number;
		onCancel: () => void;
	}

	let { elapsedMs, onCancel }: Props = $props();
</script>

<div
	class="voice-locked-bar flex w-full items-center gap-2 ps-3 pe-2"
	data-testid="voice-locked-bar"
>
	<RecordingIndicator {elapsedMs} micSize={18} />

	<div class="flex-1"></div>

	<button
		type="button"
		class="px-2 py-1 text-base font-medium text-red-500 active:opacity-60"
		onclick={onCancel}
		aria-label={m.voiceCancel()}
		data-testid="voice-cancel"
	>
		{m.cancel()}
	</button>
</div>
