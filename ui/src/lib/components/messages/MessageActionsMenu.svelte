<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiContentCopy, mdiPencilOutline } from '@mdi/js';
	import { List } from 'konsta/svelte';
	import ListAction from '$lib/components/navigation/ListAction.svelte';

	interface Props {
		/** Whether to offer an edit action (author, within the edit window). */
		canEdit?: boolean;
		onEdit?: () => void;
		onCopy: () => void;
	}

	let { canEdit = false, onEdit, onCopy }: Props = $props();
</script>

<List nested data-testid="message-actions-menu">
	{#if canEdit}
		<ListAction
			title={m.edit()}
			icon={mdiPencilOutline}
			onClick={() => onEdit?.()}
			data-testid="message-action-edit"
		/>
	{/if}
	<ListAction
		title={m.menuCopy()}
		icon={mdiContentCopy}
		onClick={onCopy}
		data-testid="message-action-copy"
	/>
</List>
