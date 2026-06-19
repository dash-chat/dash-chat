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
		...rest
	}: Props = $props();
</script>

<Button
	clear
	inline
	{onClick}
	{...rest}
	aria-label={label}
	aria-expanded={expanded}
	data-testid={testid}
	class="!rounded-full !p-0 !text-inherit opacity-60 transition hover:opacity-90 {className}"
>
	<wa-icon class={iconClass} src={wrapPathInSvg(icon)}></wa-icon>
</Button>
