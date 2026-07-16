<script lang="ts">
	import { condenseReactions } from '$lib/utils/emojis';
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Block, Button, Chip } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import type { Message, DeviceId, MessagesStore } from 'dash-chat-stores';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import SpotlightOverlay from '$lib/components/SpotlightOverlay.svelte';
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
	}

	let {
		message,
		opened = $bindable(),
		target,
		myDeviceId,
		canEdit = false,
		onEdit,
	}: Props = $props();

	const store: MessagesStore = getContext('messages-store');

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

	const condensed = $derived(condenseReactions(message.reactions, myDeviceId));
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
