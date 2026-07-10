<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiPlus, mdiClose } from '@mdi/js';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick?: () => void;
		/** Reflects the open state of the menu/panel the caller opens. */
		expanded?: boolean;
		testid?: string;
		class?: string;
	}

	let {
		onClick = () => {},
		expanded = false,
		testid = 'message-input-attach',
		class: className = '',
	}: Props = $props();
</script>

<IconButton
	icon={expanded ? mdiClose : mdiPlus}
	{onClick}
	{expanded}
	{testid}
	label={m.attachMenu()}
	class={className}
/>
