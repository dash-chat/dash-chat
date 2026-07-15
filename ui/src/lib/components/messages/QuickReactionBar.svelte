<script lang="ts">
	import { QUICK_EMOJIS } from '$lib/utils/emojis';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiDotsHorizontal } from '@mdi/js';
	import { Button } from 'konsta/svelte';
	import type { Message, DeviceId } from 'dash-chat-stores';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
		onReact: (emoji: string) => void;
		/** Open the full emoji picker. */
		onExpand: () => void;
	}

	let { message, myDeviceId, onReact, onExpand }: Props = $props();

	function hasReacted(emoji: string): boolean {
		return message.reactions[myDeviceId] === emoji;
	}
</script>

<div
	class="flex items-center gap-1 px-1 py-0.5"
	role="group"
	aria-label={m.quickReactions()}
	data-testid="quick-reaction-bar"
>
	{#each QUICK_EMOJIS as emoji}
		<Button
			clear
			inline
			onClick={() => onReact(emoji)}
			class="!h-9 !w-9 !rounded-full !p-0 text-xl transition-transform hover:scale-110 {hasReacted(
				emoji,
			)
				? '!bg-blue-100 dark:!bg-blue-900'
				: ''}"
			data-testid={`quick-reaction-${emoji}`}
		>
			{emoji}
		</Button>
	{/each}
	<IconButton
		icon={mdiDotsHorizontal}
		onClick={onExpand}
		label={m.moreReactions()}
		testid="quick-reaction-more"
		iconClass="text-xl"
	/>
</div>
