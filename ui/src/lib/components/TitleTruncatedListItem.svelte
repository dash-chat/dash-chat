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
		media: _media,
		after: _after,
		subtitle: _subtitle,
		inner: _inner,
		content: _content,
		children,
		titleWrapClass = '',
		...rest
	}: Props = $props();
</script>

<ListItem
	titleWrapClass={[titleWrapClass]
		.filter(Boolean)
		.join(' ')}
	{...rest}
>
	{#snippet media()}{#if _media}{@render _media()}{/if}{/snippet}
	{#snippet after()}{#if _after}{@render _after()}{/if}{/snippet}
	{#snippet subtitle()}{#if _subtitle}{@render _subtitle()}{/if}{/snippet}
	{#snippet inner()}{#if _inner}{@render _inner()}{/if}{/snippet}
	{#snippet content()}{#if _content}{@render _content()}{/if}{/snippet}
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
