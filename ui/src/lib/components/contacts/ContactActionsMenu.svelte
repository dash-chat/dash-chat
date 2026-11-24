<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { List, Popover } from 'konsta/svelte';
	import { mdiCancel } from '@mdi/js';
	import type { AgentId } from 'dash-chat-stores';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import BlockContactDialog from './block/BlockContactDialog.svelte';

	interface Props {
		/** Element the menu opens below. */
		anchor: HTMLElement;
		/** Which edge of the anchor the menu aligns to. */
		align: 'start' | 'end';
		agentId: AgentId;
		name: string;
		/** Called after the menu has fully closed. */
		onClose: () => void;
	}

	let { anchor, align, agentId, name, onClose }: Props = $props();

	let blockDialogOpen = $state(false);

	const position = $derived.by(() => {
		const rect = anchor.getBoundingClientRect();
		const isRtl = document.dir === 'rtl';
		const startInset = isRtl ? window.innerWidth - rect.right : rect.left;
		const endInset = isRtl ? rect.left : window.innerWidth - rect.right;
		// 16 = the list item's horizontal padding, so the menu's start edge
		// lines up with the row's content rather than the screen edge.
		return align === 'start'
			? {
					style: `top: ${rect.bottom}px; inset-inline-start: ${startInset + 16}px`,
					origin: isRtl ? '!origin-top-right' : '!origin-top-left',
				}
			: {
					style: `top: ${rect.bottom}px; inset-inline-end: ${endInset}px`,
					origin: isRtl ? '!origin-top-left' : '!origin-top-right',
				};
	});

	function closeOnDismiss(dialogOpened: boolean) {
		if (!dialogOpened) onClose();
	}
</script>

<!-- Open for the component's whole life: modal.close() plays the exit
     animation and onClosed lets the owner unmount. The block dialog only
     hides the popover, so the modal (and this component) stay up under it. -->
<Modal opened onClosed={onClose}>
	{#snippet children(modal)}
		<!-- No Konsta `target`: its positioner can only center the popover on
		     an anchor, so the menu is placed here instead, below the anchor. -->
		<Popover
			opened={modal.opened && !blockDialogOpen}
			backdrop
			onBackdropClick={modal.close}
			style={position.style}
			class="!w-auto !min-w-44 [&>div]:!rounded-2xl {position.origin}"
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
	{/snippet}
</Modal>

<BlockContactDialog
	bind:opened={() => blockDialogOpen, closeOnDismiss}
	{agentId}
	{name}
/>
