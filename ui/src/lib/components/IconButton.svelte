<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { Snippet } from 'svelte';
	import { Button, Preloader } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';

	interface Props {
		icon?: string;
		label: string;
		testid?: string;
		/** For toggle buttons: announced as aria-expanded. Omit for plain buttons. */
		expanded?: boolean;
		/** Give the button a translucent surface background. */
		filled?: boolean;
		loading?: boolean;
		iconClass?: string;
		class?: string;
		children?: Snippet;

		onClick?: (event: MouseEvent & { currentTarget: HTMLElement }) => void;
		onPointerDown?: (event: PointerEvent) => void;
		onPointerMove?: (event: PointerEvent) => void;
		onPointerUp?: (event: PointerEvent) => void;
		onPointerCancel?: (event: PointerEvent) => void;
	}

	let {
		icon,
		onClick,
		onPointerDown,
		onPointerMove,
		onPointerUp,
		onPointerCancel,
		label,
		testid,
		expanded,
		filled = false,
		loading = false,
		iconClass = 'text-2xl',
		class: className = '',
		children,
	}: Props = $props();

	const filledClass = $derived(
		filled
			? '!bg-black/10 hover:!bg-black/15 dark:!bg-white/10 dark:hover:!bg-white/20'
			: '',
	);

	function click(event: MouseEvent & { currentTarget: HTMLElement }) {
		event.preventDefault();
		event.stopPropagation();
		onClick?.(event);
	}

	// Keeps a press off an ancestor’s own gesture (e.g. a bubble’s long-press).
	// Svelte delegates pointerdown to the root, so stopping propagation would also
	// swallow a forwarded `onPointerDown` — hence never for a button that has one.
	const stopAncestorPress = $derived(
		onPointerDown ? undefined : (event: PointerEvent) => event.stopPropagation(),
	);
</script>

<Button
	clear
	inline
	onClick={click}
	onpointerdowncapture={stopAncestorPress}
	onpointerdown={onPointerDown}
	onpointermove={onPointerMove}
	onpointerup={onPointerUp}
	onpointercancel={onPointerCancel}
	aria-label={label}
	aria-expanded={expanded}
	data-testid={testid}
	style="width: 2.5rem; height: 2.5rem"
	class="!rounded-full !p-0 !text-inherit opacity-60 transition hover:bg-black/10 dark:hover:bg-white/10 {filledClass} {className}"
>
	{#if loading}
		<Preloader class="h-6 w-6" />
	{:else if children}
		{@render children()}
	{:else if icon}
		<wa-icon class={iconClass} src={wrapPathInSvg(icon)}></wa-icon>
	{/if}
</Button>
