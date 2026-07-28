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
		canDelete?: boolean;
		onDelete?: () => void;
		/** Names this mount. A desktop message hosts two of these menus at once —
		 * the hover toolbar's and the right-click one — so they need distinct
		 * ids for tests to resolve the one that is actually open. */
		testid?: string;
	}

	let {
		canEdit = false,
		onEdit,
		onCopy,
		canDelete = false,
		onDelete,
		testid = 'message-actions-menu',
	}: Props = $props();
</script>

<List nested data-testid={testid}>
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
	{#if canDelete}
		<ListAction
			title={m.delete()}
			icon={mdiDelete}
			actionType="danger"
			onClick={() => onDelete?.()}
			data-testid="message-action-delete"
		/>
	{/if}
</List>
