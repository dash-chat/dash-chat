<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { ListItem } from 'konsta/svelte';

	interface Props {
		value: File | null;
	}

	let { value = $bindable(null) }: Props = $props();

	let filePicker: HTMLInputElement;

	function onFileSelected() {
		value = filePicker.files?.[0] ?? null;
	}
</script>

<input
	type="file"
	accept="image/*"
	bind:this={filePicker}
	class="hidden"
	onchange={onFileSelected}
/>

{#if value}
	<ListItem
		title={value.name}
		link
		onClick={() => {
			value = null;
			filePicker.value = '';
		}}
	>
		{#snippet after()}
			<span class="text-red-500">{m.removeImage()}</span>
		{/snippet}
	</ListItem>
{:else}
	<ListItem title={m.selectImage()} link onClick={() => filePicker.click()} />
{/if}
