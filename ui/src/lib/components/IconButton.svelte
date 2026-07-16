<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { Snippet } from 'svelte';
	import { Button } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';

	interface Props {
		icon?: string;
		onClick: () => void;
		label: string;
		testid?: string;
		/** For toggle buttons: announced as aria-expanded. Omit for plain buttons. */
		expanded?: boolean;
		/** Give the button a translucent surface background. */
		filled?: boolean;
		class?: string;
		children?: Snippet;
	}

	let {
		icon,
		onClick,
		label,
		testid,
		expanded,
		filled = false,
		class: className = '',
		children,
	}: Props = $props();

	const filledClass = $derived(
		filled
			? '!bg-black/10 hover:!bg-black/15 dark:!bg-white/10 dark:hover:!bg-white/20'
			: '',
	);
</script>

<Button
	clear
	inline
	{onClick}
	aria-label={label}
	aria-expanded={expanded}
	data-testid={testid}
	style="width: 2.5rem; height: 2.5rem"
	class="!rounded-full !p-0 !text-inherit opacity-60 transition hover:bg-black/10 dark:hover:bg-white/10 {filledClass} {className}"
>
	{#if children}
		{@render children()}
	{:else if icon}
		<wa-icon class="text-2xl" src={wrapPathInSvg(icon)}></wa-icon>
	{/if}
</Button>
