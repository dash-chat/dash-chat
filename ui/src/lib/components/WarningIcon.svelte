<!-- The warning sign, in the colour of the app's list icons unless `color`
     (Tailwind text classes) says otherwise. -->
<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiAlert } from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { wrapPathInSvg } from '$lib/utils/icon';

	let {
		size,
		color,
		class: className,
		...rest
	}: { size: number; color?: string } & HTMLAttributes<HTMLElement> = $props();

	const theme = $derived(useTheme());
</script>

<wa-icon
	{...rest}
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
