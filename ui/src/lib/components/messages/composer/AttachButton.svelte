<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiPlus } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick?: () => void;
		/** Reflects the open state of the menu/panel the caller opens. */
		expanded?: boolean;
		class?: string;
		iconClass?: string;
	}

	let {
		onClick = () => {},
		expanded = false,
		class: className = '',
		iconClass = '',
	}: Props = $props();
</script>

<!-- The plus rotates into an X instead of swapping to a close icon: changing
     wa-icon's src loads the new SVG asynchronously, which blanks the icon the
     first time the menu opens. -->
<IconButton
	{onClick}
	{expanded}
	testid="message-input-attach"
	label={m.attachMenu()}
	class={className}
>
	<wa-icon
		class="text-2xl {iconClass} transition-transform duration-200 {expanded
			? 'rotate-45'
			: ''}"
		src={wrapPathInSvg(mdiPlus)}
	></wa-icon>
</IconButton>
