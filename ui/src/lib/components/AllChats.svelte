<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/button/button.js';
	import '@awesome.me/webawesome/dist/components/badge/badge.js';
	import '@awesome.me/webawesome/dist/components/relative-time/relative-time.js';
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { ChatsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { Badge, List } from 'konsta/svelte';
	import TitleTruncatedListItem from './TitleTruncatedListItem.svelte';
	import {
		moreThanAnHourAgo,
		lessThanAMinuteAgo,
		inYesterday,
		beforeYesterday,
	} from '$lib/utils/time';
	import { useTheme } from 'konsta/svelte';
	import { page } from '$app/state';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from './profiles/Avatar.svelte';

	let { class: className = '' }: { class?: string } = $props();

	const chatsStore: ChatsStore = getContext('chats-store');
	const chatSummaries = useReactivePromise(chatsStore.allChatsSummaries);

	const chatHref = (summary: { type: string; chatId: string }) =>
		summary.type === 'GroupChat'
			? `/group-chat/${summary.chatId}`
			: `/direct-chats/${summary.chatId}`;

	let activePath = $derived(page.url.pathname);

	const isActive = (summary: { type: string; chatId: string }) =>
		isWideScreen.value && activePath.startsWith(chatHref(summary));
	const theme = $derived(useTheme());
</script>

<div class={className}>
	{#await $chatSummaries then summaries}
		{#if summaries.length > 0}
			<List
				nested
				inset={isWideScreen.value && theme === 'ios'}
				data-testid="all-chats-list"
			>
				{#each summaries as summary}
					<TitleTruncatedListItem
						title={summary.name}
						link
						class={isActive(summary) ? 'active' : ''}
						linkProps={{ href: chatHref(summary) }}
						chevron={false}
					>
						{#snippet media()}
							<Avatar
								image={summary.avatar}
								initials={summary.name.slice(0, 2)}
							/>
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
					</TitleTruncatedListItem>
				{/each}
			</List>
		{:else}
			<div
				class="quiet flex flex-1 flex-col items-center justify-center pb-16 text-center"
				data-testid="all-chats-empty"
			>
				<p>{m.noChatsYet()}</p>
				<p>{m.noChatsYetSubtitle()}</p>
			</div>
		{/if}
	{/await}
</div>
