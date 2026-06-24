<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiTrashCanOutline } from '@mdi/js';
	import IconButton from '$lib/components/IconButton.svelte';
	import RecordingIndicator from './RecordingIndicator.svelte';

	interface Props {
		/** Live elapsed time while recording hands-free. */
		elapsedMs: number;
		/** Discard the recording. */
		onCancel: () => void;
	}

	let { elapsedMs, onCancel }: Props = $props();
</script>

<div
	class="voice-locked-bar flex w-full items-center gap-2 px-1"
	data-testid="voice-locked-bar"
>
	<IconButton
		icon={mdiTrashCanOutline}
		onClick={onCancel}
		label={m.voiceCancel()}
		testid="voice-cancel"
		class="h-10 w-10 shrink-0"
	/>

	<div class="flex flex-1 items-center gap-2">
		<RecordingIndicator {elapsedMs} micSize={18} />
	</div>
</div>
