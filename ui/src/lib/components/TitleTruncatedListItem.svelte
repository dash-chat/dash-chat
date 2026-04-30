<script lang="ts">
	import { ListItem } from 'konsta/svelte';
	import type { Snippet } from 'svelte';

	type ListItemProps = InstanceType<typeof ListItem>['$$prop_def'];

	interface Props extends ListItemProps {
		'data-testid'?: string;
		titleWrapClass?: string;
		media?: Snippet;
		after?: Snippet;
		subtitle?: Snippet;
		inner?: Snippet;
		content?: Snippet;
		children?: Snippet;
	}

	let {
		media,
		after,
		subtitle,
		inner,
		content,
		children,
		titleWrapClass = '',
		...rest
	}: Props = $props();
</script>

<ListItem
	titleWrapClass={['title-truncated-wrap', titleWrapClass]
		.filter(Boolean)
		.join(' ')}
	{media}
	{after}
	{subtitle}
	{inner}
	{content}
	{...rest}
>
	{#if children}{@render children()}{/if}
</ListItem>

<style>
	:global(.title-truncated-wrap > div:first-child) {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		width: 0;
		flex-shrink: 1;
		flex-grow: 1;
		margin-right: 8px;
	}
</style>
