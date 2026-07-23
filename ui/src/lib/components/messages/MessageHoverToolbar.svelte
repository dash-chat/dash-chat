<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiDotsHorizontal, mdiHeartPlusOutline } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onReact: (anchor: HTMLElement) => void;
		onMenu: (anchor: HTMLElement) => void;
		/** Flip the visual order so the ⋯ button sits away from the bubble. */
		reverse?: boolean;
	}

	let { onReact, onMenu, reverse = false }: Props = $props();

	let reactEl = $state<HTMLElement>();
	let menuEl = $state<HTMLElement>();
</script>

<div
	class="flex items-center gap-0.5 {reverse ? 'flex-row-reverse' : ''}"
	data-testid="message-hover-toolbar"
>
	<span bind:this={reactEl}>
		<IconButton
			onClick={() => {
				if (reactEl) onReact(reactEl);
			}}
			label={m.addReaction()}
			testid="message-hover-react"
			class="!h-9 !w-9"
		>
			<wa-icon class="text-xl" src={wrapPathInSvg(mdiHeartPlusOutline)}
			></wa-icon>
		</IconButton>
	</span>
	<span bind:this={menuEl}>
		<IconButton
			onClick={() => {
				if (menuEl) onMenu(menuEl);
			}}
			label={m.messageOptions()}
			testid="message-hover-menu"
			class="!h-9 !w-9"
		>
			<wa-icon class="text-xl" src={wrapPathInSvg(mdiDotsHorizontal)}></wa-icon>
		</IconButton>
	</span>
</div>
