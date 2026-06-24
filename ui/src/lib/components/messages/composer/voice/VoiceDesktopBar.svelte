<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiTrashCanOutline } from '@mdi/js';
	import IconButton from '$lib/components/IconButton.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';
	import RecordingIndicator from './RecordingIndicator.svelte';

	interface Props {
		/** Live elapsed time while recording. */
		elapsedMs: number;
		/** Discard the recording. */
		onCancel: () => void;
		/** Stop the recording and send it immediately. */
		onSend: () => void;
	}

	let { elapsedMs, onCancel, onSend }: Props = $props();
</script>

<div
	class="flex w-full items-center gap-3 px-2"
	data-testid="voice-desktop-bar"
>
	<IconButton
		icon={mdiTrashCanOutline}
		onClick={onCancel}
		label={m.voiceCancel()}
		testid="voice-cancel"
		class="h-10 w-10 shrink-0"
	/>

	<RecordingIndicator
		{elapsedMs}
		micSize={18}
		timerTestid="voice-recording-timer"
	/>

	<span class="flex-1 truncate text-sm opacity-60">{m.voiceRecording()}</span>

	<SendButton disabled={false} onClick={onSend} testid="voice-send" />
</div>
