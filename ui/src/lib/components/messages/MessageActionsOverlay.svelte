<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import {
		type Message,
		type DeviceId,
		type MessagesStore,
		hasBody,
	} from 'dash-chat-stores';
	import { useReactivePromise } from '$lib/stores/use-signal';
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
		onReply?: () => void;
	}

	let {
		message,
		opened = $bindable(),
		target,
		myDeviceId,
		onEdit,
		onReply,
	}: Props = $props();

	const store: MessagesStore = getContext('messages-store');
	const readOnly = useReactivePromise(store.readOnly);

	let expanded = $state(false);

	// Reset the picker state once the actions UI is closed.
	$effect(() => {
		if (!opened) expanded = false;
	});

	function close() {
		opened = false;
	}

	function react(emoji: string) {
		toggleReaction(store, message, emoji);
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

{#snippet reactionBar()}
	<QuickReactionBar
		{message}
		onReact={react}
		onExpand={() => (expanded = true)}
	/>
{/snippet}

{#await $readOnly then readOnly}
	<SpotlightOverlay
		{opened}
		{target}
		onClose={close}
		contentHidden={expanded}
		above={readOnly ? undefined : reactionBar}
	>
		{#snippet below()}
			<MessageActionsMenu
				{message}
				{myDeviceId}
				onEdit={edit}
				onReply={onReply ? reply : undefined}
				onCopy={copy}
				onDelete={close}
			/>
		{/snippet}
	</SpotlightOverlay>
{/await}

{#if opened}
	<ExpandedReactionsSheet {message} opened={expanded} onReact={react} />
{/if}
