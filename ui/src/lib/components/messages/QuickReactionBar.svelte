<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { QUICK_EMOJIS } from '$lib/utils/emojis';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiDotsHorizontal } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { type Message, type DeviceId, hasBody } from 'dash-chat-stores';
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
		if (!hasBody(message.content)) return false;
		return message.content.reactions[myDeviceId] === emoji;
	}
</script>

<div
	class="flex items-center gap-1 px-1 py-0.5"
	role="group"
	aria-label={m.quickReactions()}
	data-testid="quick-reaction-bar"
>
	{#each QUICK_EMOJIS as emoji}
		<button
			class="flex h-9 w-9 items-center justify-center rounded-full text-xl transition-transform hover:scale-110 {hasReacted(
				emoji,
			)
				? 'bg-blue-100 dark:bg-blue-900'
				: ''}"
			onclick={() => onReact(emoji)}
			data-testid={`quick-reaction-${emoji}`}
		>
			{emoji}
		</button>
	{/each}
	<IconButton
		onClick={onExpand}
		label={m.moreReactions()}
		testid="quick-reaction-more"
		class="!h-9 !w-9"
	>
		<wa-icon class="text-xl" src={wrapPathInSvg(mdiDotsHorizontal)}></wa-icon>
	</IconButton>
</div>
