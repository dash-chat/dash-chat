<script lang="ts">
	import { condenseReactions } from '$lib/utils/emojis';
	import { Block, Button, Chip } from 'konsta/svelte';
	import { type Message, hasBody } from 'dash-chat-stores';
	import EmojiPickerSheet from './EmojiPickerSheet.svelte';
	import { useMyAgentId } from '$lib/stores/my-agent-id';

	interface Props {
		message: Message;
		opened: boolean;
		onReact: (emoji: string) => void;
		onClose: () => void;
	}

	let { message, opened, onReact, onClose }: Props = $props();

	const myAgentId = useMyAgentId();

	const condensed = $derived(
		condenseReactions(
			hasBody(message.content) ? message.content.reactions : {},
			myAgentId,
		),
	);
</script>

<EmojiPickerSheet
	{opened}
	{onClose}
	backdrop={false}
	onEmojiSelected={onReact}
	testid="expanded-reactions-picker"
>
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
</EmojiPickerSheet>
