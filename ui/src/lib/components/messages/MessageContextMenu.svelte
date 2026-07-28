<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Popover } from 'konsta/svelte';
	import type { Message, DeviceId } from 'dash-chat-stores';
	import { canEditMessage } from './message-helpers';
	import MessageActionsMenu from './MessageActionsMenu.svelte';
	import { writeText } from '$lib/utils/clipboard';
	import { showToast } from '$lib/utils/toasts';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
		onEdit?: () => void;
		/** Viewport point to anchor the menu at; undefined = closed. Bindable so
		 * the caller opens it from its own right-click/long-press handler. */
		point?: { x: number; y: number };
	}

	let { message, myDeviceId, onEdit, point = $bindable() }: Props = $props();

	// Depends on `point` so the 24h edit window is re-checked each time the
	// menu opens, not once at mount.
	const canEdit = $derived(
		point !== undefined && canEditMessage(message, myDeviceId),
	);

	let pointAnchorEl = $state<HTMLElement>();

	$effect(() => {
		if (point === undefined) return;
		// Konsta popovers only re-read their anchors on window resize; nudge
		// them once the anchor is in place.
		requestAnimationFrame(() => window.dispatchEvent(new Event('resize')));
	});

	function close() {
		point = undefined;
	}

	function onKeydown(e: KeyboardEvent) {
		if (point !== undefined && e.key === 'Escape') close();
	}

	// The popover anchor is fixed, so it would visibly detach from a
	// scrolling message — dismiss instead, like Signal.
	function onScroll() {
		if (point !== undefined) close();
	}

	function onOutsideContextMenu(e: MouseEvent) {
		if (e.target instanceof Element && !e.target.closest('.k-popover')) {
			e.preventDefault();
			close();
		}
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

<svelte:window onkeydowncapture={onKeydown} onscrollcapture={onScroll} />

{#if point}
	<!-- Viewport-pixel anchor; --k-safe-area-top zeroes out the space above
	     it so Konsta places the popover below the point, context-menu style. -->
	<div
		bind:this={pointAnchorEl}
		class="pointer-events-none fixed"
		style={`left: ${point.x}px; top: ${point.y}px; width: 0; height: 0; --k-safe-area-top: ${point.y}px`}
	></div>
{/if}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="contents [&>div:not(.k-popover)]:!bg-transparent"
	oncontextmenu={onOutsideContextMenu}
>
	<Popover
		opened={point !== undefined && pointAnchorEl !== undefined}
		target={pointAnchorEl}
		backdrop
		onBackdropClick={close}
		class="!w-auto !min-w-44 [&>div]:!rounded-2xl"
	>
		<MessageActionsMenu
			{canEdit}
			onEdit={edit}
			onCopy={copy}
			testid="message-context-menu"
		/>
	</Popover>
</div>
