<script lang="ts">
	import type {
		ChatId,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { useReactivePromises } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import {
		connectionModes,
		connectionStatus,
		UNKNOWN_CONNECTION_MODES,
		type ConnectionModes,
		type ConnectionStatus,
	} from './connection-status';
	import type { ConnectionStatusTag } from './connection-status-tags';
	import { m } from '$lib/paraglide/messages.js';
	import { onActivate } from '$lib/utils/keyboard';
	import ConnectionModeDialog from './ConnectionModeDialog.svelte';
	import ConnectionStatusIcon from './ConnectionStatusIcon.svelte';

	/** Counts the peers online in `chatId`, or in every chat without one. */
	let { chatId }: { chatId?: ChatId } = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const onlinePeersStore: IOnlinePeersStore = getContext('online-peers-store');

	const inputs = useReactivePromises(() => [
		mailboxTrackerStore.connectionStatusForAllMailboxes(),
		chatId === undefined
			? onlinePeersStore.onlinePeersInAnyChat()
			: onlinePeersStore.onlinePeers(chatId),
	]);

	const ariaLabels: Record<ConnectionStatusTag, () => string> = {
		cloud: m.connectionStatusCloudAriaLabel,
		pending: m.connectionStatusPendingAriaLabel,
		local: m.connectionStatusLocalAriaLabel,
		p2p: m.connectionStatusP2pAriaLabel,
		disconnected: m.connectionStatusDisconnectedAriaLabel,
	};

	let dialogOpened = $state(false);
</script>

{#snippet chip(status: ConnectionStatus, modes: ConnectionModes)}
	<span
		data-testid="connection-status"
		data-status={status.status}
		class="flex cursor-pointer"
		role="button"
		tabindex={0}
		aria-label={ariaLabels[status.status]()}
		onclick={() => (dialogOpened = true)}
		onkeydown={onActivate(() => (dialogOpened = true))}
	>
		<span class="flex w-[35px] justify-center" aria-hidden="true">
			<ConnectionStatusIcon {status} />
		</span>
	</span>

	<ConnectionModeDialog
		bind:opened={dialogOpened}
		isolated={status.status === 'disconnected'}
		{modes}
	/>
{/snippet}

{#await $inputs}
	{@render chip({ status: 'pending' }, UNKNOWN_CONNECTION_MODES)}
{:then [mailboxes, onlinePeers]}
	{@render chip(
		connectionStatus(
			mailboxes,
			onlinePeers.length > 0,
			renderingResumedAt.value,
		),
		connectionModes(mailboxes, onlinePeers.length, renderingResumedAt.value),
	)}
{:catch}
	<!-- A store that stalls rejects here, and can still resolve later. -->
	{@render chip({ status: 'pending' }, UNKNOWN_CONNECTION_MODES)}
{/await}
