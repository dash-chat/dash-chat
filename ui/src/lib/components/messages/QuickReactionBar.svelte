<script lang="ts">
	import { QUICK_EMOJIS } from '$lib/utils/emojis';
	import { mdiPlus } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { Message, DeviceId } from 'dash-chat-stores';

	interface Props {
		message: Message;
		targetElement: HTMLElement;
		opened: boolean;
		isOwnMessage: boolean;
		myDeviceId: DeviceId;
		onReaction: (emoji: string) => void;
		onExpand: () => void;
		onClose: () => void;
	}

	let {
		message,
		targetElement,
		opened,
		isOwnMessage,
		myDeviceId,
		onReaction,
		onExpand,
		onClose,
	}: Props = $props();

	let barStyle = $derived.by(() => {
		if (!opened || !targetElement) return '';
		const rect = targetElement.getBoundingClientRect();
		const barWidth = 320;
		let left: number;

		if (isOwnMessage) {
			left = rect.right - barWidth;
		} else {
			left = rect.left;
		}

		// Clamp to viewport
		left = Math.max(8, Math.min(left, window.innerWidth - barWidth - 8));

		const top = rect.top - 52;
		const finalTop = top < 8 ? rect.bottom + 8 : top;

		return `left: ${left}px; top: ${finalTop}px;`;
	});

	function hasReacted(emoji: string): boolean {
		return message.reactions[myDeviceId] === emoji;
	}
</script>

{#if opened}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 z-50" onclick={onClose} oncontextmenu={(e) => { e.preventDefault(); onClose(); }}>
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div
			class="fixed z-50 flex items-center gap-1 rounded-full bg-white px-2 py-1.5 shadow-lg dark:bg-gray-800"
			style={barStyle}
			onclick={(e) => e.stopPropagation()}
		>
			{#each QUICK_EMOJIS as emoji}
				<button
					class="flex h-9 w-9 items-center justify-center rounded-full text-xl transition-transform hover:scale-110 {hasReacted(emoji) ? 'bg-blue-100 dark:bg-blue-900' : ''}"
					onclick={() => onReaction(emoji)}
				>
					{emoji}
				</button>
			{/each}
			<button
				class="flex h-9 w-9 items-center justify-center rounded-full text-gray-500 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-gray-700"
				onclick={onExpand}
				aria-label="More reactions"
			>
				<wa-icon src={wrapPathInSvg(mdiPlus)} style="font-size: 1.25rem"></wa-icon>
			</button>
		</div>
	</div>
{/if}
