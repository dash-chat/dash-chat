<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { Button } from 'konsta/svelte';
	import { mdiMicrophone } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { showToast } from '$lib/utils/toasts';

	interface Props {
		onCancel: () => void;
		onSend: () => void;
	}

	let { onCancel, onSend }: Props = $props();

	let stream: MediaStream | undefined;
	let recorder: MediaRecorder | undefined;

	onMount(async () => {
		try {
			stream = await navigator.mediaDevices.getUserMedia({ audio: true });
			recorder = new MediaRecorder(stream);
			recorder.start();
		} catch (e) {
			console.error('Failed to start voice recording', e);
			showToast(m.errorMicrophoneAccess(), 'error', e);
			onCancel();
		}
	});

	onDestroy(() => {
		if (recorder && recorder.state !== 'inactive') recorder.stop();
		stream?.getTracks().forEach(track => track.stop());
	});
</script>

<div class="row w-full items-center gap-2 px-2">
	<wa-icon
		class="shrink-0 text-2xl text-red-500"
		src={wrapPathInSvg(mdiMicrophone)}
	></wa-icon>
	<span class="grow"></span>
	<Button
		rounded
		clear
		inline
		onClick={onCancel}
		data-testid="voice-composer-cancel"
	>
		{m.cancel()}
	</Button>
	<Button rounded inline onClick={onSend} data-testid="voice-composer-send">
		{m.send()}
	</Button>
</div>
