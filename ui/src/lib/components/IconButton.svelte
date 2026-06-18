<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Button } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';

	type Variant = 'ghost' | 'overlay';

	interface Props {
		icon: string;
		onClick: () => void;
		label: string;
		testid?: string;
		expanded?: boolean;
		/** `ghost`: neutral icon on the page surface (composer). `overlay`: white
		 * icon over a dark backdrop (lightbox). */
		variant?: Variant;
		iconClass?: string;
		class?: string;
	}

	let {
		icon,
		onClick,
		label,
		testid,
		expanded,
		variant = 'ghost',
		iconClass = 'text-2xl',
		class: className = '',
	}: Props = $props();

	const variantClass: Record<Variant, string> = {
		ghost: '!p-0 !text-inherit opacity-60 hover:opacity-90',
		overlay: '!p-2 !text-white opacity-85 hover:opacity-100',
	};
</script>

<Button
	clear
	inline
	{onClick}
	aria-label={label}
	aria-expanded={expanded}
	data-testid={testid}
	class="!rounded-full transition {variantClass[variant]} {className}"
>
	<wa-icon class={iconClass} src={wrapPathInSvg(icon)}></wa-icon>
</Button>
