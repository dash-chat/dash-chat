<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiPlus } from '@mdi/js';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick?: () => void;
		/** Reflects the open state of the menu/panel the caller opens. */
		expanded?: boolean;
		testid?: string;
		class?: string;
		iconClass?: string;
	}

	let {
		onClick = () => {},
		expanded = false,
		testid = 'message-input-attach',
		class: className = '',
		iconClass,
	}: Props = $props();
</script>

<!-- The plus rotates into an X instead of swapping to a close icon: changing
     wa-icon's src loads the new SVG asynchronously, which blanks the icon the
     first time the menu opens. -->
<IconButton
	icon={mdiPlus}
	{onClick}
	{expanded}
	{testid}
	label={m.attachMenu()}
	class={className}
	iconClass="{iconClass ??
		'text-2xl'} transition-transform duration-200 {expanded ? 'rotate-45' : ''}"
/>
