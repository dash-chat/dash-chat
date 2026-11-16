<script lang="ts">
	import internetConnectedSvg from '$lib/assets/connection-mode/internet-connected.svg?raw';
	import internetFailedSvg from '$lib/assets/connection-mode/internet-failed.svg?raw';
	import internetSearchingSvg from '$lib/assets/connection-mode/internet-searching.svg?raw';
	import localHubConnectedSvg from '$lib/assets/connection-mode/local-hub-connected.svg?raw';
	import localHubSearchingSvg from '$lib/assets/connection-mode/local-hub-searching.svg?raw';
	import phoneToPhoneConnectedSvg from '$lib/assets/connection-mode/phone-to-phone-connected.svg?raw';
	import phoneToPhoneSearchingSvg from '$lib/assets/connection-mode/phone-to-phone-searching.svg?raw';
	import isolationSvg from '$lib/assets/connection-status/isolation.svg?raw';
	import Modal from '$lib/components/Modal.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { Dialog } from 'konsta/svelte';
	import type { ConnectionModes, InternetStatus } from './connection-status';
	import type { ConnectionModeStatus } from './connection-status-tags';

	let {
		opened = $bindable(false),
		isolated,
		modes,
	}: { opened: boolean; isolated: boolean; modes: ConnectionModes } = $props();

	const internetModeStatuses: Record<InternetStatus, ConnectionModeStatus> = {
		connected: 'connected',
		pending: 'searching',
		disconnected: 'failed',
	};
	const internetIcons: Record<InternetStatus, string> = {
		connected: internetConnectedSvg,
		pending: internetSearchingSvg,
		disconnected: internetFailedSvg,
	};
	const internetDescriptions: Record<InternetStatus, () => string> = {
		connected: m.connectionModeConnected,
		pending: m.connectionModeTryingToConnect,
		disconnected: m.connectionModeInternetNoAccess,
	};

	const localHubConnected = $derived(modes.reachableLocalHubsCount > 0);
	const phoneToPhoneConnected = $derived(modes.onlinePeersCount > 0);

	function closeOnEscape(event: KeyboardEvent) {
		if (opened && event.key === 'Escape') opened = false;
	}
</script>

<svelte:window onkeydown={closeOnEscape} />

{#snippet mode(
	id: string,
	status: ConnectionModeStatus | undefined,
	icon: string,
	title: string,
	description: string,
	iconClass = 'size-11',
)}
	<div
		class="flex items-center gap-3"
		data-testid="connection-mode-{id}"
		data-status={status}
	>
		<span class="grid size-11 shrink-0 place-items-center" aria-hidden="true">
			<span class="block {iconClass} *:size-full">
				{@html icon}
			</span>
		</span>
		<div class="flex min-w-0 flex-col text-sm/5">
			<span>{title}</span>
			<span class="opacity-70">{description}</span>
		</div>
	</div>
{/snippet}

<Modal bind:opened>
	{#snippet children(modal)}
		<Dialog
			data-testid="connection-status-dialog"
			opened={modal.opened}
			onBackdropClick={modal.close}
			title={m.connectionModeTitle()}
		>
			<div class="flex flex-col gap-4 text-start">
				{#if isolated}
					{@render mode(
						'isolation',
						undefined,
						isolationSvg,
						m.connectionModeIsolationTitle(),
						m.connectionModeIsolationDescription(),
						'size-6',
					)}
				{/if}
				{@render mode(
					'internet',
					internetModeStatuses[modes.internet],
					internetIcons[modes.internet],
					m.connectionModeInternetTitle(),
					internetDescriptions[modes.internet](),
				)}
				{@render mode(
					'local-hub',
					localHubConnected ? 'connected' : 'searching',
					localHubConnected ? localHubConnectedSvg : localHubSearchingSvg,
					m.connectionModeLocalHubTitle(),
					localHubConnected
						? m.connectionModeLocalHubConnected({
								count: modes.reachableLocalHubsCount,
							})
						: m.connectionModeTryingToConnect(),
				)}
				{@render mode(
					'phone-to-phone',
					phoneToPhoneConnected ? 'connected' : 'searching',
					phoneToPhoneConnected
						? phoneToPhoneConnectedSvg
						: phoneToPhoneSearchingSvg,
					m.connectionModePhoneToPhoneTitle(),
					phoneToPhoneConnected
						? m.connectionModePhoneToPhoneFound({
								count: modes.onlinePeersCount,
							})
						: m.connectionModeTryingToConnect(),
				)}
			</div>
		</Dialog>
	{/snippet}
</Modal>
