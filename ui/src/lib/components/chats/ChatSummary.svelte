<script lang="ts">
	import '@awesome.me/webawesome/dist/components/badge/badge.js';
	import '@awesome.me/webawesome/dist/components/relative-time/relative-time.js';
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { type ChatSummary } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { Badge } from 'konsta/svelte';
	import TitleTruncatedListItem from '../TitleTruncatedListItem.svelte';
	import {
		moreThanAnHourAgo,
		lessThanAMinuteAgo,
		inYesterday,
		beforeYesterday,
	} from '$lib/utils/time';
	import { groupEventText } from '$lib/utils/group-event-text';
	import Avatar from '../profiles/Avatar.svelte';

	let { summary, active }: { summary: ChatSummary; active: boolean } = $props();

	const chatHref = (s: ChatSummary) =>
		s.type === 'GroupChat'
			? `/group-chat/${s.chatId}`
			: `/direct-chats/${s.chatId}`;
</script>

<TitleTruncatedListItem
	title={summary.name || m.waitingForProfile()}
	titleWrapClass={summary.name ? '' : 'quiet'}
	link
	class={active ? 'active' : ''}
	linkProps={{ href: chatHref(summary) }}
	chevron={false}
	data-testid="all-chats-row"
>
	{#snippet media()}
		<Avatar image={summary.avatar} initials={summary.name.slice(0, 2)} />
	{/snippet}
	{#snippet after()}
		<span class="text-xs">
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
					style="text-align: end"
					format="narrow"
					date={new Date(summary.lastEvent.timestamp)}
				>
				</wa-relative-time>
			{/if}
		</span>
	{/snippet}
	{#snippet subtitle()}
		<div class="row items-center">
			<span class="flex-1 min-w-0 truncate text-black/70 dark:text-white/70">
				{#if summary.lastEvent.kind === 'contact_request'}
					{m.messageRequest()}
				{:else if summary.lastEvent.kind === 'contact_added'}
					{m.contactAccepted()}
				{:else if summary.lastEvent.kind === 'message'}
					{#if summary.type === 'GroupChat'}
						<strong>{summary.lastEvent.authorName || m.someone()}</strong>:
						{summary.lastEvent.text}
					{:else}
						{summary.lastEvent.text}
					{/if}
				{:else}
					{groupEventText(summary.lastEvent)}
				{/if}
			</span>
			{#if summary.unreadMessages !== 0}
				<Badge>{summary.unreadMessages}</Badge>
			{/if}
		</div>
	{/snippet}
</TitleTruncatedListItem>
