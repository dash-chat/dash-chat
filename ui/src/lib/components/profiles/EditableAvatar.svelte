<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Button } from 'konsta/svelte';
	import Avatar from './Avatar.svelte';

	interface Props {
		image?: string;
		initials?: string;
		onEdit: () => void;
		class?: string;
	}

	let { image, initials, onEdit, class: className }: Props = $props();
</script>

<div
	class="column gap-2 items-center {className ?? ''}"
	data-testid="editable-avatar"
>
	<Avatar {image} {initials} size={100} />
	<Button
		tonal
		style="width: auto"
		rounded
		small
		onClick={onEdit}
		data-testid="edit-photo">{m.editPhoto()}</Button
	>
</div>
