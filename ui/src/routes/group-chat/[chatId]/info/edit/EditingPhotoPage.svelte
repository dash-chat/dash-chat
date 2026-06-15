<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Page } from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import AvatarPicker from '$lib/components/profiles/AvatarPicker.svelte';

	let {
		avatar = $bindable(),
		onConfirm,
		onCancel,
	}: {
		avatar: string | undefined;
		onConfirm: () => void;
		onCancel: () => void;
	} = $props();

	let inModalState = $state(false);
</script>

<Page>
	<AvatarPicker
		bind:avatar
		bind:inModalState
		onClose={onCancel}
		onSave={onConfirm}
		saveLabel={m.save()}
	/>

	{#if !inModalState && !isIos}
		<Button rounded tonal onClick={onConfirm} class="fixed-action-btn">
			{m.save()}
		</Button>
	{/if}
</Page>
