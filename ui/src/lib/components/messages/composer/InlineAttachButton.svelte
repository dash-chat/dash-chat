<script lang="ts">
	import { fly } from 'svelte/transition';
	import AttachButton from '$lib/components/messages/composer/AttachButton.svelte';

	interface Props {
		onClick?: () => void;
	}

	let { onClick = () => {} }: Props = $props();
</script>

<div
	class="flex"
	transition:fly={{ x: document.dir === 'rtl' ? -16 : 16, duration: 150 }}
>
	<AttachButton {onClick} class="me-1" />
</div>
