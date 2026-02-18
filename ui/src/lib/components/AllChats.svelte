<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/button/button.js';
	import '@awesome.me/webawesome/dist/components/badge/badge.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import '@awesome.me/webawesome/dist/components/relative-time/relative-time.js';
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { ChatsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { Badge, List, ListItem } from 'konsta/svelte';
	import {
		moreThanAnHourAgo,
		lessThanAMinuteAgo,
		inYesterday,
		beforeYesterday,
	} from '$lib/utils/time';

	const chatsStore: ChatsStore = getContext('chats-store');
	const chatSummaries = useReactivePromise(chatsStore.allChatsSummaries);
</script>

<List nested data-testid="all-chats-list">
	{#await $chatSummaries then summaries}
		{#each summaries as summary}
			<ListItem
				title={summary.name}
				link
				linkProps={{
					href:
						summary.type === 'GroupChat'
							? `/group-chat/${summary.chatId}`
							: `/direct-chats/${summary.chatId}`,
				}}
				chevron={false}
			>
				{#snippet media()}
					<wa-avatar image={summary.avatar} initials={summary.name.slice(0, 2)}>
					</wa-avatar>
				{/snippet}
				{#snippet after()}
					{#if beforeYesterday(summary.lastEvent.timestamp)}
						<wa-format-date
							weekday="short"
							date={new Date(summary.lastEvent.timestamp)}
						></wa-format-date>
					{:else if inYesterday(summary.lastEvent.timestamp)}
						{m.yesterday().toLocaleLowerCase()}
					{:else if lessThanAMinuteAgo(summary.lastEvent.timestamp)}
						{m.now()}
					{:else if moreThanAnHourAgo(summary.lastEvent.timestamp)}
						<wa-format-date
							hour="numeric"
							minute="numeric"
							hour-format="24"
							date={new Date(summary.lastEvent.timestamp)}
						></wa-format-date>
					{:else}
						<wa-relative-time
							sync
							style="text-align: right"
							format="narrow"
							date={new Date(summary.lastEvent.timestamp)}
						>
						</wa-relative-time>
					{/if}
				{/snippet}
				{#snippet subtitle()}
					<div class="row" style="align-items: center">
						<span style="flex: 1"
							>{summary.type === 'ContactRequest'
								? m.messageRequest()
								: summary.lastEvent.summary === 'contact_added'
									? m.contactAccepted()
									: summary.lastEvent.summary}</span
						>
						{#if summary.unreadMessages !== 0}
							<Badge>{summary.unreadMessages}</Badge>
						{/if}
					</div>
				{/snippet}
			</ListItem>
		{:else}
			<ListItem title={m.noChatsYet()} data-testid="all-chats-empty" />
		{/each}
	{/await}
</List>

<style>
</style>
