<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiDotsHorizontal, mdiHeartPlusOutline } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { Popover } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import type { Message, DeviceId, MessagesStore } from 'dash-chat-stores';
	import { canEditMessage } from './message-helpers';
	import IconButton from '$lib/components/IconButton.svelte';
	import QuickReactionBar from './QuickReactionBar.svelte';
	import MessageActionsMenu from './MessageActionsMenu.svelte';
	import ExpandedReactionsSheet from './ExpandedReactionsSheet.svelte';
	import { toggleReaction } from '$lib/utils/reactions';
	import { writeText } from '$lib/utils/clipboard';
	import { showToast } from '$lib/utils/toasts';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
		onEdit?: () => void;
		/** Flip the visual order so the ⋯ button sits away from the bubble. */
		reverse?: boolean;
	}

	let { message, myDeviceId, onEdit, reverse = false }: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	const canEdit = $derived(canEditMessage(message, myDeviceId));

	let open = $state<'reactions' | 'menu' | null>(null);
	let anchor = $state<HTMLElement | { x: number; y: number }>();
	let expanded = $state(false);
	let reactEl = $state<HTMLElement>();
	let menuEl = $state<HTMLElement>();
	let pointAnchorEl = $state<HTMLElement>();

	const point = $derived(anchor instanceof HTMLElement ? undefined : anchor);
	const targetEl = $derived(
		anchor instanceof HTMLElement ? anchor : pointAnchorEl,
	);

	/** Open the actions menu popover at a viewport point (e.g. the cursor of a
	 * right-click on the bubble). */
	export function openMenuAt(p: { x: number; y: number }) {
		anchor = p;
		open = 'menu';
	}

	// Reset the picker state once the actions UI is closed.
	$effect(() => {
		if (open === null) expanded = false;
	});

	$effect(() => {
		if (open === null) return;
		// Konsta popovers only re-read their anchors on window resize; nudge
		// them once the anchor is in place.
		requestAnimationFrame(() => window.dispatchEvent(new Event('resize')));
	});

	function close() {
		open = null;
	}

	function onKeydown(e: KeyboardEvent) {
		if (open !== null && e.key === 'Escape') close();
	}

	// The popover anchors are fixed, so it would visibly detach from a
	// scrolling message — dismiss instead, like Signal.
	function onScroll() {
		if (open !== null) close();
	}

	function onOutsideContextMenu(e: MouseEvent) {
		if (e.target instanceof Element && !e.target.closest('.k-popover')) {
			e.preventDefault();
			close();
		}
	}

	function react(emoji: string) {
		toggleReaction(store, message, myDeviceId, emoji);
		close();
	}

	function edit() {
		close();
		onEdit?.();
	}

	async function copy() {
		close();
		await writeText(message.content.message);
		showToast(m.copiedMessageToClipboard());
	}
</script>

<svelte:window onkeydowncapture={onKeydown} onscrollcapture={onScroll} />

<div
	class="absolute {reverse
		? 'end-full me-1'
		: 'start-full ms-1'} inset-y-0 flex items-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100 {open !==
	null
		? '!opacity-100'
		: ''}"
>
	<div
		class="flex items-center gap-0.5 {reverse ? 'flex-row-reverse' : ''}"
		data-testid="message-hover-toolbar"
	>
		<span bind:this={reactEl}>
			<IconButton
				onClick={() => {
					anchor = reactEl;
					open = 'reactions';
				}}
				label={m.addReaction()}
				testid="message-hover-react"
				class="!h-9 !w-9"
			>
				<wa-icon class="text-xl" src={wrapPathInSvg(mdiHeartPlusOutline)}
				></wa-icon>
			</IconButton>
		</span>
		<span bind:this={menuEl}>
			<IconButton
				onClick={() => {
					anchor = menuEl;
					open = 'menu';
				}}
				label={m.messageOptions()}
				testid="message-hover-menu"
				class="!h-9 !w-9"
			>
				<wa-icon class="text-xl" src={wrapPathInSvg(mdiDotsHorizontal)}
				></wa-icon>
			</IconButton>
		</span>
	</div>
</div>

{#if open !== null && point}
	<!-- Viewport-pixel anchor; --k-safe-area-top zeroes out the space above
	     it so Konsta places the popover below the point, context-menu style. -->
	<div
		bind:this={pointAnchorEl}
		class="pointer-events-none fixed"
		style={`left: ${point.x}px; top: ${point.y}px; width: 0; height: 0; --k-safe-area-top: ${point.y}px`}
	></div>
{/if}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="contents [&>div:not(.k-popover)]:!bg-transparent"
	oncontextmenu={onOutsideContextMenu}
>
	<Popover
		opened={open === 'reactions' && targetEl !== undefined}
		target={targetEl}
		backdrop
		onBackdropClick={close}
		class="!w-auto !rounded-full {expanded ? 'invisible' : ''}"
	>
		<QuickReactionBar
			{message}
			{myDeviceId}
			onReact={react}
			onExpand={() => (expanded = true)}
		/>
	</Popover>
	<Popover
		opened={open === 'menu' && targetEl !== undefined}
		target={targetEl}
		backdrop
		onBackdropClick={close}
		class="!w-auto !min-w-44 [&>div]:!rounded-2xl"
	>
		<MessageActionsMenu {canEdit} onEdit={edit} onCopy={copy} />
	</Popover>
</div>

{#if open !== null}
	<ExpandedReactionsSheet
		{message}
		{myDeviceId}
		opened={expanded}
		onReact={react}
	/>
{/if}
