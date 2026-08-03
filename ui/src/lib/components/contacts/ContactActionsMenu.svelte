<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { List, Popover } from 'konsta/svelte';
	import { mdiCancel } from '@mdi/js';
	import type { AgentId } from 'dash-chat-stores';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import BlockContactDialog from './block/BlockContactDialog.svelte';

	interface Props {
		/** Element the menu hangs off. */
		anchor: HTMLElement;
		agentId: AgentId;
		name: string;
		onClose: () => void;
	}

	let { anchor, agentId, name, onClose }: Props = $props();

	let blockDialogOpen = $state(false);

	function closeOnDismiss(opened: boolean) {
		if (!opened) onClose();
	}
</script>

<Popover
	opened={!blockDialogOpen}
	target={anchor}
	backdrop
	onBackdropClick={onClose}
	class="!w-auto !min-w-44 [&>div]:!rounded-2xl"
>
	<List nested data-testid="contact-actions-menu">
		<ListAction
			title={m.block()}
			icon={mdiCancel}
			actionType="danger"
			onClick={() => (blockDialogOpen = true)}
			data-testid="contact-block"
		/>
	</List>
</Popover>

<BlockContactDialog
	bind:opened={() => blockDialogOpen, closeOnDismiss}
	{agentId}
	{name}
/>
