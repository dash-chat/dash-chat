<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiContentCopy, mdiDelete, mdiPencilOutline } from '@mdi/js';
	import { List } from 'konsta/svelte';
	import ListAction from '$lib/components/navigation/ListAction.svelte';

	interface Props {
		/** Whether to offer an edit action (author, within the edit window). */
		canEdit?: boolean;
		onEdit?: () => void;
		onCopy: () => void;
		/** Whether to offer a delete-for-everyone action (author, within the delete window). */
		canDeleteForEveryone?: boolean;
		onDelete?: () => void;
	}

	let {
		canEdit = false,
		onEdit,
		onCopy,
		canDeleteForEveryone = false,
		onDelete,
	}: Props = $props();
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
	{#if canDeleteForEveryone}
		<ListAction
			title={m.delete()}
			icon={mdiDelete}
			actionType="danger"
			onClick={() => onDelete?.()}
			data-testid="message-action-delete"
		/>
	{/if}
</List>
