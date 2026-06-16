<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Button } from 'konsta/svelte';
	import Avatar from './Avatar.svelte';

	interface Props {
		image?: string;
		name?: string;
		colorSeed?: string;
		group?: boolean;
		onEdit: () => void;
		class?: string;
	}

	let {
		image,
		name,
		colorSeed,
		group = false,
		onEdit,
		class: className,
	}: Props = $props();
</script>

<div class="column gap-2 items-center {className ?? ''}">
	<Avatar {image} {name} {colorSeed} {group} size={100} />
	<Button
		tonal
		style="width: auto"
		rounded
		small
		onClick={onEdit}
		data-testid="edit-photo">{m.editPhoto()}</Button
	>
</div>
