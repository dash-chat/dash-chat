<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiShare } from '$lib/utils/icon';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick: () => void;
		label?: string;
		testid?: string;
		class?: string;
	}

	let {
		onClick,
		label = m.share(),
		testid,
		class: className = '',
	}: Props = $props();
</script>

<IconButton icon={mdiShare} {onClick} {label} {testid} class={className} />
