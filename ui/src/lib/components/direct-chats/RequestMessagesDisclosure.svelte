<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Button } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiChevronDown, mdiChevronUp } from '@mdi/js';

	interface Props {
		count: number;
		revealed: boolean;
	}

	let { count, revealed = $bindable() }: Props = $props();
</script>

<div class="row justify-center px-4" data-testid="direct-chat-request-messages">
	<Button
		rounded
		tonal
		small
		inline
		class="gap-1"
		data-testid="direct-chat-request-messages-toggle"
		onClick={() => (revealed = !revealed)}
	>
		{m.requestMessagesReceived({ count })}
		<wa-icon
			class="small-icon"
			src={wrapPathInSvg(revealed ? mdiChevronUp : mdiChevronDown)}
		></wa-icon>
	</Button>
</div>
