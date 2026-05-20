<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { getContext } from 'svelte';
	import { mdiCheckCircleOutline } from '@mdi/js';
	import {
		type DeviceId,
		type MailboxTrackerStore,
		type TopicId as ChatId,
	} from 'dash-chat-stores';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';

	import SendingSpinner from './SendingSpinner.svelte';

	interface Props {
		chatId: ChatId;
		author: DeviceId;
		seq: number;
	}

	const props: Props = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);

	const syncStatus = useReactivePromise(
		mailboxTrackerStore.syncStatusForOp,
		props.chatId,
		props.author,
		props.seq,
	);
</script>

{#await $syncStatus then syncStatus}
	{#if syncStatus.syncedWithCloudMailbox}
		<wa-icon
			data-testid="message-status"
			data-status="cloud"
			class="message-status"
			src={wrapPathInSvg(mdiCheckCircleOutline)}
			aria-label="sent"
		></wa-icon>
	{:else if syncStatus.syncedWithAnyLocalMailbox}
		<wa-icon
			data-testid="message-status"
			data-status="local"
			class="message-status"
			src="/localmailboxserver.svg"
			aria-label="sent-to-local-mailboxes"
		></wa-icon>
	{:else}
		<div
			data-testid="message-status"
			data-status="sending"
			class="message-status"
		>
			<SendingSpinner />
		</div>
	{/if}
{/await}

<style>
	.message-status {
		opacity: 0.7;
		font-size: 0.875rem;
		width: 0.875rem;
		height: 0.875rem;
	}
</style>
