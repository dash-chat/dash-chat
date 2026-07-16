<script lang="ts">
	import { fly } from 'svelte/transition';
	import AttachButton from '$lib/components/messages/composer/AttachButton.svelte';

	interface Props {
		onClick?: () => void;
		/** Reflects the open state of the menu/panel the caller opens. */
		expanded?: boolean;
	}

	let { onClick = () => {}, expanded = false }: Props = $props();
</script>

<div
	class="flex"
	transition:fly={{ x: document.dir === 'rtl' ? -16 : 16, duration: 150 }}
>
	<AttachButton {onClick} {expanded} class="me-1" />
</div>
