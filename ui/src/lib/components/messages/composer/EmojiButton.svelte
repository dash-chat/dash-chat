<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiEmoticonHappyOutline } from '@mdi/js';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick: () => void;
	}

	let { onClick }: Props = $props();
</script>

<IconButton
	icon={mdiEmoticonHappyOutline}
	{onClick}
	label={m.emoji()}
	testid="message-input-emoji"
	iconClass="text-[22px]"
	class="h-7 w-7"
/>
