<script lang="ts">
	import { mdiExportVariant, mdiShareVariant } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { isIos } from '$lib/utils/environment';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick: () => void;
		label?: string;
		testid?: string;
		circle?: boolean;
		iconClass?: string;
		class?: string;
	}

	let {
		onClick,
		label = m.share(),
		testid,
		circle = false,
		iconClass,
		class: className = '',
	}: Props = $props();
</script>

<IconButton
	icon={isIos ? mdiExportVariant : mdiShareVariant}
	{onClick}
	{label}
	{testid}
	{circle}
	{iconClass}
	class={className}
/>
