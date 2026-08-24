<script lang="ts">
	import { Sheet, Dialog, List } from 'konsta/svelte';
	import type { DeviceId } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { condenseReactions } from '$lib/utils/emojis';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import ReactionsSheetRow from './ReactionsSheetRow.svelte';

	let {
		reactions,
		myDeviceId,
		onToggleReaction,
		opened = $bindable(),
	}: {
		reactions: Record<DeviceId, string>;
		myDeviceId: DeviceId;
		onToggleReaction: (emoji: string) => void;
		opened: boolean;
	} = $props();

	const entries = $derived(
		Object.entries(reactions) as Array<[DeviceId, string]>,
	);
	const condensed = $derived(condenseReactions(reactions, myDeviceId));

	let filter = $state<string | null>(null);

	const filtered = $derived(
		filter === null ? entries : entries.filter(([, emoji]) => emoji === filter),
	);

	$effect(() => {
		if (!opened) filter = null;
	});

	function close() {
		opened = false;
	}

	function removeOwn(emoji: string) {
		onToggleReaction(emoji);
		close();
	}

	function tabClass(active: boolean): string {
		return `flex items-center gap-1 rounded-full px-3 py-1 text-sm ${
			active
				? 'bg-gray-200 dark:bg-gray-600 border border-gray-400 dark:border-gray-400'
				: 'border border-transparent'
		}`;
	}
</script>

{#snippet content()}
	<div class="flex flex-wrap items-center gap-1.5 px-3 pt-3" role="tablist">
		<button
			role="tab"
			aria-selected={filter === null}
			class={tabClass(filter === null)}
			onclick={() => (filter = null)}
			data-testid="reactions-tab-all"
		>
			{m.reactionsAll()} · {entries.length}
		</button>
		{#each condensed as reaction}
			<button
				role="tab"
				aria-selected={filter === reaction.emoji}
				class={tabClass(filter === reaction.emoji)}
				onclick={() => (filter = reaction.emoji)}
				data-testid={`reactions-tab-${reaction.emoji}`}
			>
				{reaction.emoji}
				{reaction.count}
			</button>
		{/each}
	</div>
	<List class="!my-2">
		{#each filtered as [deviceId, emoji] (deviceId)}
			{@const own = deviceId === myDeviceId}
			<ReactionsSheetRow
				{deviceId}
				{emoji}
				{own}
				removable={own && !isWideScreen.value}
				onRemove={() => removeOwn(emoji)}
			/>
		{/each}
	</List>
{/snippet}

<Modal bind:opened>
	{#snippet children(modal)}
		{#if isWideScreen.value}
			<Dialog opened={modal.opened} onBackdropClick={close} class="!p-0">
				<div data-testid="reactions-sheet">
					{@render content()}
				</div>
			</Dialog>
		{:else}
			<Sheet class="pb-safe" opened={modal.opened} onBackdropClick={close}>
				<div data-testid="reactions-sheet">
					<div class="flex flex-col items-center">
						<SheetHandle />
					</div>
					{@render content()}
				</div>
			</Sheet>
		{/if}
	{/snippet}
</Modal>
