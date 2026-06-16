<script lang="ts">
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { ListItem } from 'konsta/svelte';

	type ActionType = 'primary' | 'normal' | 'danger';

	type Props = {
		title: string;
		actionType?: ActionType;
		onClick?: () => void;
		icon?: string;
		href?: string;
		'data-testid'?: string;
	};

	const actionColors = {
		primary: {
			primaryTextIos: 'text-blue-500',
			primaryTextMaterial: 'text-blue-600',
		},
		normal: {},
		danger: {
			primaryTextIos: 'text-red-500',
			primaryTextMaterial: 'text-red-600',
		},
	};

	let {
		title,
		actionType = 'normal',
		onClick,
		icon,
		href,
		'data-testid': testId,
	}: Props = $props();
</script>

<ListItem
	{title}
	link
	chevron={false}
	{onClick}
	linkProps={href ? { href } : undefined}
	data-testid={testId}
	colors={actionColors[actionType]}
>
	{#snippet media()}
		{#if icon}
			<wa-icon src={wrapPathInSvg(icon)}></wa-icon>
		{/if}
	{/snippet}
</ListItem>
