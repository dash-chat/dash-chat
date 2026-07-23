<script lang="ts">
	import { condenseReactions } from '$lib/utils/emojis';
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Block, Button, Chip, Popover } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import {
		type Message,
		type DeviceId,
		type MessagesStore,
		hasBody,
	} from 'dash-chat-stores';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import SpotlightOverlay from '$lib/components/SpotlightOverlay.svelte';
	import { isMobile } from '$lib/utils/environment';
	import EmojiPickerWrapper from './EmojiPickerWrapper.svelte';
	import QuickReactionBar from './QuickReactionBar.svelte';
	import MessageActionsMenu from './MessageActionsMenu.svelte';
	import { toggleReaction } from '$lib/utils/reactions';
	import { writeText } from '$lib/utils/clipboard';
	import { showToast } from '$lib/utils/toasts';

	interface Props {
		message: Message;
		/** Whether the actions UI is showing — drives the overlay open/close. */
		opened: boolean;
		/** The message bubble the overlay anchors to. */
		target: HTMLElement | undefined;
		myDeviceId: DeviceId;
		/** Whether to offer an edit action (author, within the edit window). */
		canEdit?: boolean;
		onEdit?: () => void;
		/** Whether to offer a delete-for-everyone action (author, within the delete window). */
		canDeleteForEveryone?: boolean;
		onDelete?: () => void;
		/** Which desktop popover is showing; null = closed. Desktop only. */
		desktopOpen?: 'reactions' | 'menu' | null;
		/** Anchor for the desktop popover: a toolbar button or a cursor point. */
		desktopAnchor?: HTMLElement | { x: number; y: number };
	}

	let {
		message,
		opened = $bindable(),
		target,
		myDeviceId,
		canEdit = false,
		onEdit,
		canDeleteForEveryone = false,
		onDelete,
		desktopOpen = $bindable(null),
		desktopAnchor,
	}: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	let expanded = $state(false);
	let pointAnchorEl = $state<HTMLElement>();

	const point = $derived(
		desktopAnchor instanceof HTMLElement ? undefined : desktopAnchor,
	);
	const desktopTarget = $derived(
		desktopAnchor instanceof HTMLElement ? desktopAnchor : pointAnchorEl,
	);

	// Reset the picker state once the actions UI is closed.
	$effect(() => {
		if (!opened && desktopOpen === null) expanded = false;
	});

	$effect(() => {
		if (desktopOpen === null) return;
		// Konsta popovers only re-read their anchors on window resize; nudge
		// them once the anchor is in place.
		requestAnimationFrame(() => window.dispatchEvent(new Event('resize')));
	});

	function close() {
		opened = false;
		desktopOpen = null;
	}

	function onDesktopKeydown(e: KeyboardEvent) {
		if (desktopOpen !== null && e.key === 'Escape') close();
	}

	// The popover anchors are fixed, so it would visibly detach from a
	// scrolling message — dismiss instead, like Signal.
	function onDesktopScroll() {
		if (desktopOpen !== null) close();
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

	function del() {
		close();
		onDelete?.();
	}

	async function copy() {
		close();
		if (!hasBody(message.content)) return;
		await writeText(message.content.message);
		showToast(m.copiedMessageToClipboard());
	}

	const condensed = $derived(
		condenseReactions(
			hasBody(message.content) ? message.content.reactions : {},
			myDeviceId,
		),
	);
</script>

<svelte:window
	onkeydowncapture={onDesktopKeydown}
	onscrollcapture={onDesktopScroll}
/>

{#if isMobile}
	<SpotlightOverlay {opened} {target} onClose={close} contentHidden={expanded}>
		{#snippet above()}
			<QuickReactionBar
				{message}
				{myDeviceId}
				onReact={react}
				onExpand={() => (expanded = true)}
			/>
		{/snippet}
		{#snippet below()}
			<MessageActionsMenu
				{canEdit}
				onEdit={edit}
				onCopy={copy}
				{canDeleteForEveryone}
				onDelete={del}
			/>
		{/snippet}
	</SpotlightOverlay>
{:else}
	{#if desktopOpen !== null && point}
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
			opened={desktopOpen === 'reactions' && desktopTarget !== undefined}
			target={desktopTarget}
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
			opened={desktopOpen === 'menu' && desktopTarget !== undefined}
			target={desktopTarget}
			backdrop
			onBackdropClick={close}
			class="!w-auto !min-w-44 [&>div]:!rounded-2xl"
		>
			<MessageActionsMenu {canEdit} onEdit={edit} onCopy={copy} />
		</Popover>
	</div>
{/if}

{#if opened || desktopOpen !== null}
	<Sheet class="pb-safe text-lg" opened={expanded} backdrop={false}>
		<div class="flex flex-col items-center">
			<SheetHandle />
		</div>
		{#if condensed.length > 0}
			<Block>
				{#each condensed as reaction}
					<Button
						clear
						inline
						class="me-2 !p-0 text-lg"
						onClick={() => react(reaction.emoji)}
					>
						<Chip class="border !border-white dark:!border-black">
							{reaction.emoji}{#if reaction.count > 1}<span class="ms-1"
									>{reaction.count}</span
								>{/if}
						</Chip>
					</Button>
				{/each}
			</Block>
		{/if}
		<Block>
			<EmojiPickerWrapper onEmojiSelected={react}></EmojiPickerWrapper>
		</Block>
	</Sheet>
{/if}
