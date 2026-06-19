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
		iconClass?: string;
		class?: string;
	}

	let {
		icon,
		onClick,
		label,
		testid,
		expanded,
		iconClass = 'text-2xl',
		class: className = '',
	}: Props = $props();
</script>

<Button
	clear
	inline
	{onClick}
	aria-label={label}
	aria-expanded={expanded}
	data-testid={testid}
	class="!rounded-full !p-0 !text-inherit opacity-60 transition hover:opacity-90 {className}"
>
	<wa-icon class={iconClass} src={wrapPathInSvg(icon)}></wa-icon>
</Button>
