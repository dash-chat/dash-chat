<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Sheet } from 'konsta/svelte';
	import { swipeToClose } from '$lib/actions/swipe-to-close';

	interface Props {
		opened: boolean;
		onClose: () => void;
		class?: string;
		style?: string;
		colors?: { bgIos?: string; bgMaterial?: string };
		backdrop?: boolean;
		children: Snippet;
	}

	let {
		opened,
		onClose,
		class: className = '',
		style,
		colors,
		backdrop = true,
		children,
	}: Props = $props();
</script>

<Sheet
	class={className}
	{style}
	{colors}
	{opened}
	{backdrop}
	onBackdropClick={onClose}
>
	<!-- `contents` keeps the sheet's own layout: children stay its direct flex items. -->
	<div class="contents" use:swipeToClose={onClose}>
		{@render children()}
	</div>
</Sheet>
