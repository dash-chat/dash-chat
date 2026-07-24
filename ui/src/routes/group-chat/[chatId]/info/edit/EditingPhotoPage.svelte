<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Page } from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import AvatarPicker from '$lib/components/profiles/AvatarPicker.svelte';
	import FixedActionButton from '$lib/components/FixedActionButton.svelte';

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
		<FixedActionButton tonal onClick={onConfirm}>
			{m.save()}
		</FixedActionButton>
	{/if}
</Page>
