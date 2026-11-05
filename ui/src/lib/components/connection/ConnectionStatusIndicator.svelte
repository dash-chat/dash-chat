<script lang="ts">
	import type {
		AgentId,
		ChatId,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { useReactivePromises } from '$lib/stores/use-signal';
	import { reactive, relay } from 'signalium';
	import { getContext, type Snippet } from 'svelte';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import { connectionStatus, type ConnectionStatus } from './connection-status';
	import type { ConnectionStatusTag } from './connection-status-tags';
	import { Dialog, DialogButton } from 'konsta/svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { onActivate } from '$lib/utils/keyboard';
	import ConnectionStatusIcon from './ConnectionStatusIcon.svelte';

	/** Without a `chatId` there is no peer to be directly connected to, so the
	 * status never reads p2p. */
	let { chatId }: { chatId?: ChatId } = $props();

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

	const titles: Record<ConnectionStatusTag, () => string> = {
		cloud: m.connectionStatusCloudTitle,
		pending: m.connectionStatusPendingTitle,
		local: m.connectionStatusLocalTitle,
		p2p: m.connectionStatusP2pTitle,
		disconnected: m.connectionStatusDisconnectedTitle,
	};

	function description(status: ConnectionStatus): string {
		switch (status.status) {
			case 'cloud':
				return m.connectionStatusCloudDescription();
			case 'pending':
				return m.connectionStatusPendingDescription();
			case 'local':
				return m.connectionStatusLocalDescription({ count: status.count });
			case 'p2p':
				return m.connectionStatusP2pDescription();
			case 'disconnected':
				return m.connectionStatusDisconnectedDescription();
		}
	}

	let dialogOpened = $state(false);

	// Konsta's Dialog type intersects with HTMLAttributes (which has `title?: string`),
	// so a Snippet for `title` can't be passed without a cast.
	const asTitle = (snippet: Snippet) => snippet as unknown as string;
</script>

{#snippet chip(status: ConnectionStatus)}
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
			<ConnectionStatusIcon status={status.status} />
		</span>
	</span>

	{#snippet titleSlot()}
		<div class="flex flex-row items-center gap-3">
			<span aria-hidden="true">
				<ConnectionStatusIcon status={status.status} />
			</span>
			<span data-testid="connection-status-dialog-title">
				{titles[status.status]()}
			</span>
		</div>
	{/snippet}

	<Modal bind:opened={dialogOpened}>
		{#snippet children(modal)}
			<Dialog
				data-testid="connection-status-dialog"
				opened={modal.opened}
				onBackdropClick={modal.close}
				title={asTitle(titleSlot)}
			>
				<span data-testid="connection-status-dialog-description">
					{description(status)}
				</span>

				{#snippet buttons()}
					<DialogButton
						data-testid="connection-status-dialog-close"
						onClick={modal.close}
					>
						{m.close()}
					</DialogButton>
				{/snippet}
			</Dialog>
		{/snippet}
	</Modal>
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
