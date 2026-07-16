<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Button } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';

	interface Props {
		icon: string;
		onClick: () => void;
		label: string;
		testid?: string;
		expanded?: boolean;
		/** Give the button a translucent surface background. */
		filled?: boolean;
		iconClass?: string;
		class?: string;
	}

	let {
		icon,
		onClick,
		label,
		testid,
		expanded,
		filled = false,
		iconClass = 'text-2xl',
		class: className = '',
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
	aria-expanded={expanded}
	data-testid={testid}
	style="width: 2.5rem; height: 2.5rem"
	class="!rounded-full !p-0 !text-inherit opacity-60 transition hover:bg-black/10 dark:hover:bg-white/10 {filledClass} {className}"
>
	<wa-icon class={iconClass} src={wrapPathInSvg(icon)}></wa-icon>
</Button>
