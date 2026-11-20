<script lang="ts">
	import '@awesome.me/webawesome/dist/components/badge/badge.js';
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import {
		type ChatSummary,
		type MessageBody,
		hasBody,
	} from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { Badge, useTheme } from 'konsta/svelte';
	import TitleTruncatedListItem from '../TitleTruncatedListItem.svelte';
	import {
		inYesterday,
		beforeYesterday,
		minutesSince,
		withinWindow,
	} from '$lib/utils/time';
	import { getLocale } from '$lib/paraglide/runtime';
	import { groupEventText } from '$lib/utils/group-event-text';
	import AvatarWithP2pBadge from './AvatarWithP2pBadge.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCancel } from '@mdi/js';

	let { summary, active }: { summary: ChatSummary; active: boolean } = $props();

	const chatHref = (s: ChatSummary) =>
		s.type === 'GroupChat'
			? `/group-chat/${s.chatId}`
			: `/direct-chats/${s.chatId}`;

	const title = $derived(summary.name || m.waitingForProfile());

	// Konsta's Material row is inset 8px from the list's edges and its iOS
	// row is not; both end up 16px from the screen edge.
	const theme = $derived(useTheme());
	const edgePadding = $derived(
		theme === 'material'
			? { content: 'ps-2!', inner: 'pe-2!' }
			: { content: 'ps-4!', inner: 'pe-4!' },
	);

	const minutesFormat = new Intl.NumberFormat(getLocale(), {
		style: 'unit',
		unit: 'minute',
		unitDisplay: 'narrow',
	});

	function summarizeMessage(content: MessageBody): string {
		if (content.message) return content.message;
		const media = content.media;
		if (!media || media.length === 0) return '';
		if (media.some(item => item.kind === 'VoiceNote')) return m.voiceMessage();
		const file = media.find(item => item.kind === 'File');
		if (file) return file.name;
		const n = media.filter(item => item.kind === 'Photo').length;
		return n > 1 ? m.photosCount({ count: n }) : m.photo();
	}
</script>

<TitleTruncatedListItem
	{title}
	link
	class={active ? 'active' : ''}
	contentClass={edgePadding.content}
	mediaClass="py-0! me-3!"
	innerClass="min-w-0 py-2! {edgePadding.inner}"
	titleWrapClass="items-baseline! min-h-0! leading-6 [&>div:first-child]:font-semibold"
	linkProps={{ href: chatHref(summary) }}
	chevron={false}
	data-testid="all-chats-row"
>
	{#snippet media()}
		<AvatarWithP2pBadge {summary} size={56} />
	{/snippet}
	{#snippet after()}
		<span class="text-[14px]/5">
			{#if beforeYesterday(summary.lastEvent.timestamp)}
				<wa-format-date
					weekday="short"
					date={new Date(summary.lastEvent.timestamp)}
				></wa-format-date>
			{:else if inYesterday(summary.lastEvent.timestamp)}
				{m.yesterday().toLocaleLowerCase()}
			{:else if !withinWindow(summary.lastEvent.timestamp, 3_600_000)}
				<wa-format-date
					hour="numeric"
					minute="numeric"
					hour-format="24"
					date={new Date(summary.lastEvent.timestamp)}
				></wa-format-date>
			{:else if minutesSince(summary.lastEvent.timestamp) < 1}
				{m.now()}
			{:else}
				{minutesFormat.format(minutesSince(summary.lastEvent.timestamp))}
			{/if}
		</span>
	{/snippet}
	{#snippet subtitle()}
		<div class="row h-10 items-start gap-2 text-[14px]/5">
			<span
				class="line-clamp-2 min-w-0 flex-1 wrap-anywhere text-black/70 dark:text-white/70"
			>
				{#if summary.blocked === true && summary.type === 'DirectChat'}
					<span class="inline-flex items-center gap-1 italic">
						<wa-icon
							class="small-icon shrink-0"
							src={wrapPathInSvg(mdiCancel)}
							data-testid="blocked-row-icon"
						></wa-icon>
						{m.blocked()}
					</span>
				{:else if summary.lastEvent.kind === 'contact_request'}
					{m.messageRequest()}
				{:else if summary.lastEvent.kind === 'contact_added'}
					{m.contactAccepted()}
				{:else if summary.lastEvent.kind === 'message'}
					{#if summary.type === 'GroupChat'}
						<span class="font-semibold"
							>{summary.lastEvent.authorName || m.someone()}:</span
						>
						{#if hasBody(summary.lastEvent.content)}
							{summarizeMessage(summary.lastEvent.content)}
						{:else}
							<span class="italic">{m.messageDeleted()}</span>
						{/if}
					{:else if hasBody(summary.lastEvent.content)}
						{summarizeMessage(summary.lastEvent.content)}
					{:else}
						<span class="italic">{m.messageDeleted()}</span>
					{/if}
				{:else}
					{groupEventText(summary.lastEvent)}
				{/if}
			</span>
			{#if summary.unreadMessages !== 0}
				<Badge data-testid="chat-row-unread-badge"
					>{summary.unreadMessages}</Badge
				>
			{/if}
		</div>
	{/snippet}
</TitleTruncatedListItem>
