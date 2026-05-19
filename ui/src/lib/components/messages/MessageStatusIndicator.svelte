<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { getContext } from 'svelte';
	import { mdiCheck } from '@mdi/js';
	import {
		PRODUCTION_MAILBOX_ID,
		type DeviceId,
		type MailboxTrackerStore,
		type TopicId as ChatId,
	} from 'dash-chat-stores';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';

	interface Props {
		chatId: ChatId;
		author: DeviceId;
		seq: number;
	}

	const props: Props = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);

	const syncedMailboxes = useReactivePromise(
		mailboxTrackerStore.syncedMailboxesForOp,
		props.chatId,
		props.author,
		props.seq,
	);
</script>

{#await $syncedMailboxes then ids}
	{#if ids.includes(PRODUCTION_MAILBOX_ID)}
		<wa-icon
			class="message-status-sent"
			src={wrapPathInSvg(mdiCheck)}
			aria-label="sent"
		></wa-icon>
	{/if}
{/await}

<style>
	.message-status-sent {
		font-size: 0.875rem;
		line-height: 1;
		opacity: 0.7;
	}
</style>
