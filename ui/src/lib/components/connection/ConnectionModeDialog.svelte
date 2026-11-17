<script lang="ts">
	import internetConnectedSvg from '$lib/assets/connection-mode/internet-connected.svg?raw';
	import internetFailedSvg from '$lib/assets/connection-mode/internet-failed.svg?raw';
	import internetSearchingSvg from '$lib/assets/connection-mode/internet-searching.svg?raw';
	import localHubConnectedSvg from '$lib/assets/connection-mode/local-hub-connected.svg?raw';
	import localHubNotFoundSvg from '$lib/assets/connection-mode/local-hub-not-found.svg?raw';
	import phoneToPhoneConnectedSvg from '$lib/assets/connection-mode/phone-to-phone-connected.svg?raw';
	import phoneToPhoneNotFoundSvg from '$lib/assets/connection-mode/phone-to-phone-not-found.svg?raw';
	import isolationSvg from '$lib/assets/connection-status/isolation.svg?raw';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import Modal from '$lib/components/Modal.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiArrowLeft, mdiChevronRight } from '@mdi/js';
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

	type ModeWithHelp = 'local-hub' | 'phone-to-phone';
	interface ModeWithHelpView {
		status: ConnectionModeStatus;
		icon: string;
		title: string;
		description: string;
		help: string[];
	}

	let helpFor: ModeWithHelp | undefined = $state();

	$effect(() => {
		if (opened) helpFor = undefined;
	});

	const localHubConnected = $derived(modes.reachableLocalHubsCount > 0);
	const phoneToPhoneConnected = $derived(modes.onlinePeersCount > 0);

	const modesWithHelp: Record<ModeWithHelp, ModeWithHelpView> = $derived({
		'local-hub': {
			status: localHubConnected ? 'connected' : 'searching',
			icon: localHubConnected ? localHubConnectedSvg : localHubNotFoundSvg,
			title: m.connectionModeLocalHubTitle(),
			description: localHubConnected
				? m.connectionModeLocalHubConnected({
						count: modes.reachableLocalHubsCount,
					})
				: m.connectionModeLocalHubNotFound(),
			help: [m.connectionModeLocalHubHelp(), m.connectionModeLocalHubWifiOff()],
		},
		'phone-to-phone': {
			status: phoneToPhoneConnected ? 'connected' : 'searching',
			icon: phoneToPhoneConnected
				? phoneToPhoneConnectedSvg
				: phoneToPhoneNotFoundSvg,
			title: m.connectionModePhoneToPhoneTitle(),
			description: phoneToPhoneConnected
				? m.connectionModePhoneToPhoneFound({ count: modes.onlinePeersCount })
				: m.connectionModePhoneToPhoneNotFound(),
			help: [
				m.connectionModePhoneToPhoneHelp(),
				m.connectionModePhoneToPhoneWifiOff(),
			],
		},
	});

	function closeOnEscape(event: KeyboardEvent) {
		if (opened && event.key === 'Escape') opened = false;
	}
</script>

<svelte:window onkeydown={closeOnEscape} />

{#snippet modeSummary(
	icon: string,
	title: string,
	description: string,
	iconClass: string,
)}
	<span class="grid size-11 shrink-0 place-items-center" aria-hidden="true">
		<span class="block {iconClass} *:size-full">
			{@html icon}
		</span>
	</span>
	<span class="flex min-w-0 flex-1 flex-col text-start text-sm/5">
		<span>{title}</span>
		<span class="opacity-70">{description}</span>
	</span>
{/snippet}

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
		{@render modeSummary(icon, title, description, iconClass)}
	</div>
{/snippet}

{#snippet modeWithHelp(id: ModeWithHelp, mode: ModeWithHelpView)}
	<button
		type="button"
		class="flex h-auto w-full cursor-pointer items-center gap-3 whitespace-normal"
		data-testid="connection-mode-{id}"
		data-status={mode.status}
		onclick={() => (helpFor = id)}
	>
		{@render modeSummary(mode.icon, mode.title, mode.description, 'size-11')}
		<wa-icon
			class="shrink-0 text-2xl opacity-70 rtl:-scale-x-100"
			src={wrapPathInSvg(mdiChevronRight)}
		></wa-icon>
	</button>
{/snippet}

{#snippet modeHelp(id: ModeWithHelp, mode: ModeWithHelpView)}
	<div
		class="flex flex-col gap-4 text-start"
		data-testid="connection-mode-{id}-help"
	>
		<button
			type="button"
			class="-ms-2 -mt-2 -mb-3 grid size-10 cursor-pointer place-items-center self-start text-2xl"
			aria-label={m.connectionModeHelpBack()}
			data-testid="connection-mode-help-back"
			onclick={() => (helpFor = undefined)}
		>
			<wa-icon class="rtl:-scale-x-100" src={wrapPathInSvg(mdiArrowLeft)}
			></wa-icon>
		</button>
		<div class="flex items-center gap-3">
			{@render modeSummary(
				mode.icon,
				mode.title,
				mode.description,
				'size-11 rounded-full bg-black/5 dark:bg-white/10',
			)}
		</div>
		{#each mode.help as paragraph}
			<p class="text-sm/5">{paragraph}</p>
		{/each}
	</div>
{/snippet}

<Modal bind:opened>
	{#snippet children(modal)}
		<Dialog
			data-testid="connection-status-dialog"
			opened={modal.opened}
			onBackdropClick={modal.close}
			title={helpFor === undefined ? m.connectionModeTitle() : undefined}
		>
			{#if helpFor !== undefined}
				{@render modeHelp(helpFor, modesWithHelp[helpFor])}
			{:else}
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
					{@render modeWithHelp('local-hub', modesWithHelp['local-hub'])}
					{@render modeWithHelp(
						'phone-to-phone',
						modesWithHelp['phone-to-phone'],
					)}
				</div>
			{/if}
		</Dialog>
	{/snippet}
</Modal>
