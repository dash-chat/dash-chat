<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { List, Popover } from 'konsta/svelte';
	import { mdiCancel } from '@mdi/js';
	import { getContext } from 'svelte';
	import type { AgentId, ContactsStore } from 'dash-chat-stores';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import BlockContactDialog from './block/BlockContactDialog.svelte';
	import UnblockContactDialog from './block/UnblockContactDialog.svelte';

	interface Props {
		/** Element the menu hangs off. */
		anchor: HTMLElement;
		agentId: AgentId;
		name: string;
		onClose: () => void;
	}

	let { anchor, agentId, name, onClose }: Props = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const blocked = $derived(
		useReactivePromise(contactsStore.isBlocked, agentId),
	);

	let phase = $state<'menu' | 'dialog'>('menu');
</script>

{#await $blocked then isBlocked}
	<Popover
		opened={phase === 'menu'}
		target={anchor}
		backdrop
		onBackdropClick={onClose}
		class="!w-auto !min-w-44 [&>div]:!rounded-2xl"
	>
		<List nested data-testid="contact-actions-menu">
			<ListAction
				title={isBlocked ? m.unblock() : m.block()}
				icon={mdiCancel}
				actionType={isBlocked ? 'normal' : 'danger'}
				onClick={() => (phase = 'dialog')}
				data-testid="contact-block-toggle"
			/>
		</List>
	</Popover>

	{#if isBlocked}
		<UnblockContactDialog
			bind:opened={() => phase === 'dialog', opened => opened || onClose()}
			{agentId}
			{name}
		/>
	{:else}
		<BlockContactDialog
			bind:opened={() => phase === 'dialog', opened => opened || onClose()}
			{agentId}
			{name}
		/>
	{/if}
{/await}
