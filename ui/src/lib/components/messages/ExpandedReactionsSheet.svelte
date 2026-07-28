<script lang="ts">
	import { condenseReactions } from '$lib/utils/emojis';
	import { Sheet, Block, Button, Chip } from 'konsta/svelte';
	import type { Message, DeviceId } from 'dash-chat-stores';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import EmojiPickerWrapper from './EmojiPickerWrapper.svelte';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
		opened: boolean;
		onReact: (emoji: string) => void;
	}

	let { message, myDeviceId, opened, onReact }: Props = $props();

	const condensed = $derived(condenseReactions(message.reactions, myDeviceId));
</script>

<Sheet class="pb-safe text-lg" {opened} backdrop={false}>
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
					onClick={() => onReact(reaction.emoji)}
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
		<EmojiPickerWrapper onEmojiSelected={onReact}></EmojiPickerWrapper>
	</Block>
</Sheet>
