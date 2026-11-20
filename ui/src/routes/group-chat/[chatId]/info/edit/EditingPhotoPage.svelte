<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Page } from 'konsta/svelte';
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
</script>

<Page>
	<AvatarPicker
		bind:avatar
		onClose={onCancel}
		onSave={onConfirm}
		saveLabel={m.save()}
	/>
</Page>
