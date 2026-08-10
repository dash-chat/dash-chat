<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { List, Popover } from 'konsta/svelte';
	import { mdiAlertOctagonOutline, mdiCancel } from '@mdi/js';
	import type { AgentId } from 'dash-chat-stores';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import BlockContactDialog from './block/BlockContactDialog.svelte';
	import ReportContactDialog from './report/ReportContactDialog.svelte';

	interface Props {
		/** Element the menu hangs off. */
		anchor: HTMLElement;
		agentId: AgentId;
		name: string;
		onClose: () => void;
	}

	let { anchor, agentId, name, onClose }: Props = $props();

	let blockDialogOpen = $state(false);
	let reportDialogOpen = $state(false);

	function closeOnDismiss(opened: boolean) {
		if (!opened) onClose();
	}
</script>

<Popover
	opened={!blockDialogOpen && !reportDialogOpen}
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
		<ListAction
			title={m.report()}
			icon={mdiAlertOctagonOutline}
			actionType="danger"
			onClick={() => (reportDialogOpen = true)}
			data-testid="contact-report"
		/>
	</List>
</Popover>

<BlockContactDialog
	bind:opened={() => blockDialogOpen, closeOnDismiss}
	{agentId}
	{name}
/>

<ReportContactDialog
	bind:opened={() => reportDialogOpen, closeOnDismiss}
	{agentId}
	{name}
/>
