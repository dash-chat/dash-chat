<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiDotsHorizontal, mdiHeartPlusOutline, mdiReply } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { Popover } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import {
		type Message,
		type DeviceId,
		type MessagesStore,
		hasBody,
	} from 'dash-chat-stores';
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
		onReply?: () => void;
		/** Flip the visual order so the ⋯ button sits away from the bubble. */
		reverse?: boolean;
	}

	let {
		message,
		myDeviceId,
		onEdit,
		onReply,
		reverse = false,
	}: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	let open = $state<'reactions' | 'menu' | null>(null);

	let expanded = $state(false);
	let reactEl = $state<HTMLElement>();
	let menuEl = $state<HTMLElement>();

	const targetEl = $derived(open === 'reactions' ? reactEl : menuEl);

	// Reset the picker state once the actions UI is closed.
	$effect(() => {
		if (open === null) expanded = false;
	});

	function close() {
		open = null;
	}

	function onKeydown(e: KeyboardEvent) {
		if (open !== null && e.key === 'Escape') close();
	}

	function onUserScroll() {
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

	function reply() {
		close();
		onReply?.();
	}

	async function copy() {
		close();
		if (!hasBody(message.content)) return;
		await writeText(message.content.message);
		showToast(m.copiedMessageToClipboard());
	}
</script>

<svelte:window
	onkeydowncapture={onKeydown}
	onwheelcapture={onUserScroll}
	ontouchmovecapture={onUserScroll}
/>

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
				onClick={() => (open = 'reactions')}
				label={m.addReaction()}
				testid="message-hover-react"
				class="!h-9 !w-9"
			>
				<wa-icon class="text-xl" src={wrapPathInSvg(mdiHeartPlusOutline)}
				></wa-icon>
			</IconButton>
		</span>
		{#if onReply}
			<IconButton
				onClick={reply}
				label={m.reply()}
				testid="message-hover-reply"
				class="!h-9 !w-9"
			>
				<wa-icon class="text-xl" src={wrapPathInSvg(mdiReply)}></wa-icon>
			</IconButton>
		{/if}
		<span bind:this={menuEl}>
			<IconButton
				onClick={() => (open = 'menu')}
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
		<MessageActionsMenu
			{message}
			{myDeviceId}
			onEdit={edit}
			onReply={onReply ? reply : undefined}
			onCopy={copy}
			onDelete={close}
		/>
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
