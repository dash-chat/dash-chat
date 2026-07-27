<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { Message, DeviceId, MessagesStore } from 'dash-chat-stores';
	import { canEditMessage } from './message-helpers';
	import SpotlightOverlay from '$lib/components/SpotlightOverlay.svelte';
	import QuickReactionBar from './QuickReactionBar.svelte';
	import MessageActionsMenu from './MessageActionsMenu.svelte';
	import ExpandedReactionsSheet from './ExpandedReactionsSheet.svelte';
	import { toggleReaction } from '$lib/utils/reactions';
	import { writeText } from '$lib/utils/clipboard';
	import { showToast } from '$lib/utils/toasts';

	interface Props {
		message: Message;
		opened: boolean;
		/** The message bubble the overlay anchors to. */
		target: HTMLElement | undefined;
		myDeviceId: DeviceId;
		onEdit?: () => void;
	}

	let {
		message,
		opened = $bindable(),
		target,
		myDeviceId,
		onEdit,
	}: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	const canEdit = $derived(canEditMessage(message, myDeviceId));

	let expanded = $state(false);

	// Reset the picker state once the actions UI is closed.
	$effect(() => {
		if (!opened) expanded = false;
	});

	function close() {
		opened = false;
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
		<MessageActionsMenu {canEdit} onEdit={edit} onCopy={copy} />
	{/snippet}
</SpotlightOverlay>

{#if opened}
	<ExpandedReactionsSheet
		{message}
		{myDeviceId}
		opened={expanded}
		onReact={react}
	/>
{/if}
