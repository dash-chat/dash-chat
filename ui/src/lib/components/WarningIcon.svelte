<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiAlert } from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { wrapPathInSvg } from '$lib/utils/icon';

	let {
		size,
		label,
		color,
		class: className,
		...rest
	}: {
		size: number;
		label?: string;
		color?: string;
	} & HTMLAttributes<HTMLElement> = $props();

	const theme = $derived(useTheme());
</script>

<wa-icon
	{...rest}
	{label}
	src={wrapPathInSvg(mdiAlert)}
	class={[
		className,
		color ??
			(theme === 'ios'
				? 'text-black dark:text-white'
				: 'text-md-light-on-surface dark:text-md-dark-on-surface'),
	]}
	style="font-size: {size}px"
></wa-icon>
