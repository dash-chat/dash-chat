<script lang="ts">
	import internetConnectedSvg from '$lib/assets/connection-mode/internet-connected.svg?raw';
	import internetFailedSvg from '$lib/assets/connection-mode/internet-failed.svg?raw';
	import internetSearchingSvg from '$lib/assets/connection-mode/internet-searching.svg?raw';
	import localHubCenteredSvg from '$lib/assets/connection-mode/local-hub-centered.svg?raw';
	import localHubConnectedSvg from '$lib/assets/connection-mode/local-hub-connected.svg?raw';
	import localHubNotFoundSvg from '$lib/assets/connection-mode/local-hub-not-found.svg?raw';
	import phoneToPhoneCenteredSvg from '$lib/assets/connection-mode/phone-to-phone-centered.svg?raw';
	import phoneToPhoneConnectedSvg from '$lib/assets/connection-mode/phone-to-phone-connected.svg?raw';
	import phoneToPhoneNotFoundSvg from '$lib/assets/connection-mode/phone-to-phone-not-found.svg?raw';
	import isolationSvg from '$lib/assets/connection-status/isolation.svg?raw';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import Modal from '$lib/components/Modal.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiArrowLeft } from '@mdi/js';
	import { Dialog, Link, List, ListItem } from 'konsta/svelte';
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

	// Konsta renders snippet titles, but types the props as strings.
	type DialogTitle = InstanceType<typeof Dialog>['$$prop_def']['title'];
	type ListItemTitle = InstanceType<typeof ListItem>['$$prop_def']['title'];

	type ModeWithHelp = 'local-hub' | 'phone-to-phone';
	interface ModeWithHelpView {
		status: ConnectionModeStatus;
		icon: string;
		/** The list's icons leave room for a status badge; the help view's
		 * circle needs the glyph in the middle. */
		helpIcon: string;
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
			helpIcon: localHubCenteredSvg,
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
			helpIcon: phoneToPhoneCenteredSvg,
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

{#snippet modeIcon(icon: string, sizeClass = 'size-11')}
	<span class="block {sizeClass} *:size-full" aria-hidden="true">
		{@html icon}
	</span>
{/snippet}

{#snippet mode(
	id: string,
	status: ConnectionModeStatus | undefined,
	icon: string,
	name: string,
	description: string,
	iconSizeClass?: string,
	onClick?: () => void,
)}
	<!-- Both lines go in the title: Konsta centres its chevron on the title,
	     and would leave it level with the first line of a subtitled item. -->
	{#snippet nameAndDescription()}
		<div>{name}</div>
		<div class="text-sm opacity-70">{description}</div>
	{/snippet}
	<!-- The dialog already pads its content; Konsta's list items add their
	     own page-edge inset on top. -->
	<ListItem
		link={onClick !== undefined}
		contentClass="ps-0!"
		mediaClass="me-1!"
		innerClass="pe-0!"
		title={nameAndDescription as ListItemTitle}
		{onClick}
		data-testid="connection-mode-{id}"
		data-status={status}
	>
		{#snippet media()}
			{@render modeIcon(icon, iconSizeClass)}
		{/snippet}
	</ListItem>
{/snippet}

{#snippet modeWithHelp(id: ModeWithHelp)}
	{@const view = modesWithHelp[id]}
	{@render mode(
		id,
		view.status,
		view.icon,
		view.title,
		view.description,
		undefined,
		() => (helpFor = id),
	)}
{/snippet}

{#snippet helpTitle()}
	{#if helpFor !== undefined}
		<span class="flex flex-col items-start gap-2">
			<!-- Konsta pads an icon link by 12px, which would indent the arrow. -->
			<Link
				iconOnly
				class="-ms-3"
				aria-label={m.connectionModeHelpBack()}
				data-testid="connection-mode-help-back"
				onClick={() => (helpFor = undefined)}
			>
				<wa-icon class="rtl:-scale-x-100" src={wrapPathInSvg(mdiArrowLeft)}
				></wa-icon>
			</Link>
			<span class="flex items-center gap-2">
				{@render modeIcon(
					modesWithHelp[helpFor].helpIcon,
					'size-11 rounded-full bg-black/5 dark:bg-white/10',
				)}
				{modesWithHelp[helpFor].title}
			</span>
		</span>
	{/if}
{/snippet}

<Modal bind:opened>
	{#snippet children(modal)}
		<!-- Wide enough for the longest status to stay on one line. -->
		<Dialog
			class="w-84!"
			data-testid="connection-status-dialog"
			opened={modal.opened}
			onBackdropClick={modal.close}
			title={helpFor === undefined
				? m.connectionModeTitle()
				: (helpTitle as DialogTitle)}
		>
			{#if helpFor !== undefined}
				<div
					class="flex flex-col gap-4"
					data-testid="connection-mode-{helpFor}-help"
				>
					{#each modesWithHelp[helpFor].help as paragraph}
						<p class="text-wrap">{paragraph}</p>
					{/each}
				</div>
			{:else}
				<List nested dividers={false}>
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
					{@render modeWithHelp('local-hub')}
					{@render modeWithHelp('phone-to-phone')}
				</List>
			{/if}
		</Dialog>
	{/snippet}
</Modal>
