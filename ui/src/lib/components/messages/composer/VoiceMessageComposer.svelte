<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Button } from 'konsta/svelte';
	import { mdiMicrophone } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';

	interface Props {
		onCancel: () => void;
		onSend: () => void;
	}

	let { onCancel, onSend }: Props = $props();
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
