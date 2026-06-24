<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Button } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	/** Pointer handlers forwarded to the underlying button — lets an icon button
	 * drive press-and-hold gestures (e.g. the voice recorder). */
	type PointerProps = Pick<
		HTMLButtonAttributes,
		'onpointerdown' | 'onpointermove' | 'onpointerup' | 'onpointercancel'
	>;

	interface Props extends PointerProps {
		icon: string;
		onClick?: () => void;
		label: string;
		testid?: string;
		expanded?: boolean;
		/** Render as a filled circular button (fixed size + translucent surface). */
		circle?: boolean;
		iconClass?: string;
		class?: string;
	}

	let {
		icon,
		onClick,
		label,
		testid,
		expanded,
		circle = false,
		iconClass = 'text-2xl',
		class: className = '',
		...rest
	}: Props = $props();

	const circleClass = $derived(
		circle
			? '!h-10 !w-10 !bg-black/10 hover:!bg-black/15 dark:!bg-white/10 dark:hover:!bg-white/20'
			: '',
	);
</script>

<Button
	clear
	inline
	{onClick}
	{...rest}
	aria-label={label}
	aria-expanded={expanded}
	data-testid={testid}
	class="!rounded-full !p-0 !text-inherit opacity-60 transition hover:opacity-90 {circleClass} {className}"
>
	<wa-icon class={iconClass} src={wrapPathInSvg(icon)}></wa-icon>
</Button>
