<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import {
		mdiContentCopy,
		mdiDeleteOutline,
		mdiPencilOutline,
		mdiReply,
	} from '@mdi/js';
	import { List } from 'konsta/svelte';
	import type { DeviceId, Message } from 'dash-chat-stores';
	import { canEditMessage } from './message-helpers';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import DeleteMessageDialog from './DeleteMessageDialog.svelte';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
		onEdit?: () => void;
		/** Start composing a reply to this message. */
		onReply?: () => void;
		onCopy: () => void;
		/** Called when the delete action is pressed, before the confirm dialog
		 * opens — the hosting popover/overlay closes itself here. */
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
		onReply,
		onCopy,
		onDelete,
		testid = 'message-actions-menu',
	}: Props = $props();

	const canEdit = $derived(canEditMessage(message, myDeviceId));

	let confirmingDelete = $state(false);
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
	<ListAction
		title={m.delete()}
		icon={mdiDeleteOutline}
		actionType="danger"
		onClick={() => {
			onDelete?.();
			confirmingDelete = true;
		}}
		data-testid="message-action-delete"
	/>
</List>

<DeleteMessageDialog {message} {myDeviceId} bind:opened={confirmingDelete} />
