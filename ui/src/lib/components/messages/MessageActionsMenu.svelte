<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiContentCopy, mdiDeleteOutline, mdiPencilOutline } from '@mdi/js';
	import { List } from 'konsta/svelte';
	import type { DeviceId, Message } from 'dash-chat-stores';
	import {
		canDeleteMessageForEveryone,
		canEditMessage,
	} from './message-helpers';
	import ListAction from '$lib/components/navigation/ListAction.svelte';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
		onEdit?: () => void;
		onCopy: () => void;
		/** Opens the delete dialog. Offered on every message, since delete-for-me
		 * is always allowed; the dialog decides whether delete-for-everyone is
		 * also on offer. */
		onDelete?: () => void;
		/** Names this mount. A desktop message hosts two of these menus at once —
		 * the hover toolbar's and the right-click one — so they need distinct
		 * ids for tests to resolve the one that is actually open. */
		testid?: string;
	}

	let {
		message,
		myDeviceId,
		onEdit,
		onCopy,
		onDelete,
		testid = 'message-actions-menu',
	}: Props = $props();

	const canEdit = $derived(canEditMessage(message, myDeviceId));
	const canDelete = $derived(canDeleteMessageForEveryone(message, myDeviceId));
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
	{#if onDelete}
		<ListAction
			title={m.delete()}
			icon={mdiDeleteOutline}
			actionType="danger"
			onClick={onDelete}
			data-testid="message-action-delete"
		/>
	{/if}
</List>
