<script lang="ts">
	import type {
		AgentId,
		ChatId,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { useReactivePromises } from '$lib/stores/use-signal';
	import { reactive, relay } from 'signalium';
	import { getContext } from 'svelte';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import { connectionStatus, type ConnectionStatus } from './connection-status';
	import type { ConnectionStatusTag } from './connection-status-tags';
	import { m } from '$lib/paraglide/messages.js';
	import { onActivate } from '$lib/utils/keyboard';
	import ConnectionStatusIcon from './ConnectionStatusIcon.svelte';

	/** Without a `chatId` there is no peer to be directly connected to, so the
	 * status never reads p2p. */
	let { chatId, onclick }: { chatId?: ChatId; onclick: () => void } = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const onlinePeersStore: IOnlinePeersStore = getContext('online-peers-store');

	// A relay, because it answers on first read: an async reactive would paint
	// the home chip as pending on every mount.
	const noPeers = reactive(() =>
		relay<AgentId[]>(state => {
			state.value = [];
		}),
	);

	const inputs = useReactivePromises(() => [
		mailboxTrackerStore.connectionStatusForAllMailboxes(),
		chatId === undefined ? noPeers() : onlinePeersStore.onlinePeers(chatId),
	]);

	const ariaLabels: Record<ConnectionStatusTag, () => string> = {
		cloud: m.connectionStatusCloudAriaLabel,
		pending: m.connectionStatusPendingAriaLabel,
		local: m.connectionStatusLocalAriaLabel,
		p2p: m.connectionStatusP2pAriaLabel,
		disconnected: m.connectionStatusDisconnectedAriaLabel,
	};
</script>

{#snippet chip(status: ConnectionStatus)}
	<span
		data-testid="connection-status"
		data-status={status.status}
		class="flex cursor-pointer"
		role="button"
		tabindex={0}
		aria-label={ariaLabels[status.status]()}
		{onclick}
		onkeydown={onActivate(onclick)}
	>
		<span class="flex w-[35px] justify-center" aria-hidden="true">
			<ConnectionStatusIcon status={status.status} />
		</span>
	</span>
{/snippet}

{#await $inputs}
	{@render chip({ status: 'pending' })}
{:then [mailboxes, onlinePeers]}
	{@render chip(
		connectionStatus(
			mailboxes,
			onlinePeers.length > 0,
			renderingResumedAt.value,
		),
	)}
{:catch}
	<!-- A store that stalls rejects here, and can still resolve later. -->
	{@render chip({ status: 'pending' })}
{/await}
