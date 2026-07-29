<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import {
		mdiContentCopy,
		mdiDelete,
		mdiPencilOutline,
		mdiReply,
	} from '@mdi/js';
	import { List } from 'konsta/svelte';
	import ListAction from '$lib/components/navigation/ListAction.svelte';

	interface Props {
		/** Whether to offer an edit action (author, within the edit window). */
		canEdit?: boolean;
		onEdit?: () => void;
		/** Start composing a reply to this message. */
		onReply?: () => void;
		onCopy: () => void;
		/** Opens the delete dialog. Offered on every message, since delete-for-me
		 * is always allowed; the dialog decides whether delete-for-everyone is
		 * also on offer. */
		onDelete?: () => void;
	}

	let { canEdit = false, onEdit, onReply, onCopy, onDelete }: Props = $props();
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
	{#if onReply}
		<ListAction
			title={m.reply()}
			icon={mdiReply}
			onClick={onReply}
			data-testid="message-action-reply"
		/>
	{/if}
	<ListAction
		title={m.menuCopy()}
		icon={mdiContentCopy}
		onClick={onCopy}
		data-testid="message-action-copy"
	/>
	{#if onDelete}
		<ListAction
			title={m.delete()}
			icon={mdiDelete}
			actionType="danger"
			onClick={onDelete}
			data-testid="message-action-delete"
		/>
	{/if}
</List>
