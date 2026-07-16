<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { Snippet } from 'svelte';
	import { Button } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';

	interface Props {
		/** mdi path rendered as the icon. Ignored when `children` is given. */
		icon?: string;
		onClick: () => void;
		label: string;
		testid?: string;
		/** Give the button a translucent surface background. */
		filled?: boolean;
		class?: string;
		/** Custom icon content; wins over `icon`. */
		children?: Snippet;
	}

	let {
		icon,
		onClick,
		label,
		testid,
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

<!-- Default 40px size as an inline style: it beats Konsta's own button height
     class by CSS precedence (not stylesheet order), while callers can still
     shrink or grow it with !important utilities (e.g. class="!h-9 !w-9"). -->
<Button
	clear
	inline
	{onClick}
	aria-label={label}
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
