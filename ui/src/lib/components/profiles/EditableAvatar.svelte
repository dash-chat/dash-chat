<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { goto } from '$app/navigation';
	import { Button } from 'konsta/svelte';
	import Avatar from './Avatar.svelte';

	interface Props {
		image?: string;
		initials?: string;
		editUrl: string;
		class?: string;
	}

	let { image, initials, editUrl, class: className }: Props = $props();
</script>

<div class="column gap-2 items-center {className ?? ''}">
	<Avatar {image} {initials} size={100} />
	<Button
		tonal
		style="width: auto"
		rounded
		small
		onClick={() => goto(editUrl)}
		data-testid="edit-photo">{m.editPhoto()}</Button
	>
</div>
