<script lang="ts">
	import { condenseReactions } from '$lib/utils/emojis';
	import { Sheet, Block, Button, Chip } from 'konsta/svelte';
	import { type Message, hasBody } from 'dash-chat-stores';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import EmojiPickerWrapper from './EmojiPickerWrapper.svelte';
	import { useMyAgentId } from '$lib/stores/my-agent-id';

	interface Props {
		message: Message;
		opened: boolean;
		onReact: (emoji: string) => void;
	}

	let { message, opened, onReact }: Props = $props();

	const myAgentId = useMyAgentId();

	const condensed = $derived(
		condenseReactions(
			hasBody(message.content) ? message.content.reactions : {},
			myAgentId,
		),
	);
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
