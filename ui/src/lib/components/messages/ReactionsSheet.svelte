<script lang="ts">
	import { Sheet, Dialog, List, ListItem, Button } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import { fullName, type AgentId, type MessagesStore } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { condenseReactions } from '$lib/utils/emojis';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useMyAgentId } from '$lib/stores/my-agent-id';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import Modal from '$lib/components/Modal.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';

	let {
		reactions,
		onToggleReaction,
		opened = $bindable(),
	}: {
		reactions: Record<AgentId, string>;
		onToggleReaction: (emoji: string) => void;
		opened: boolean;
	} = $props();

	const store: MessagesStore = getContext('messages-store');
	const myAgentId = useMyAgentId();

	const profiles = useReactivePromise(store.membersProfiles);

	const entries = $derived(
		Object.entries(reactions) as Array<[AgentId, string]>,
	);
	const condensed = $derived(condenseReactions(reactions, myAgentId));

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
</script>

{#snippet tab(target: string | null, label: string, testid: string)}
	<Button
		inline
		small
		rounded
		clear={filter !== target}
		tonal={filter === target}
		class={filter === target ? 'neutral-tonal-button' : ''}
		colors={{
			textIos: 'text-inherit',
			textMaterial: 'text-inherit',
			tonalTextIos: 'text-inherit',
			tonalTextMaterial: 'text-inherit',
		}}
		role="tab"
		aria-selected={filter === target}
		onClick={() => (filter = target)}
		data-testid={testid}
	>
		{label}
	</Button>
{/snippet}

{#snippet content()}
	<div class="flex flex-wrap items-center gap-1.5 px-3 pt-3" role="tablist">
		{@render tab(
			null,
			`${m.reactionsAll()} · ${entries.length}`,
			'reactions-tab-all',
		)}
		{#each condensed as reaction (reaction.emoji)}
			{@render tab(
				reaction.emoji,
				`${reaction.emoji} ${reaction.count}`,
				`reactions-tab-${reaction.emoji}`,
			)}
		{/each}
	</div>
	{#await $profiles then profiles}
		<List class="!my-2">
			{#each filtered as [agentId, emoji] (agentId)}
				{@const own = agentId === myAgentId}
				{@const removable = own && !isWideScreen.value}
				{@const profile = profiles[agentId]}
				<ListItem
					link={removable}
					chevron={false}
					title={own
						? m.you()
						: profile
							? fullName(profile)
							: m.unknownSender()}
					subtitle={removable ? m.tapToRemove() : undefined}
					onClick={removable ? () => removeOwn(emoji) : undefined}
					data-testid={own ? 'reaction-row-own' : 'reaction-row'}
				>
					{#snippet media()}
						{#if profile}
							<Avatar
								image={profile.avatar}
								initials={profile.name.slice(0, 2)}
								size="2.5rem"
							/>
						{:else}
							<Avatar waitingForProfile size="2.5rem" />
						{/if}
					{/snippet}
					{#snippet after()}
						<span class="text-xl">{emoji}</span>
					{/snippet}
				</ListItem>
			{/each}
		</List>
	{/await}
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
