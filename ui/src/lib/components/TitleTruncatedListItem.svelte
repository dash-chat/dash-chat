<script lang="ts">
	import { ListItem } from 'konsta/svelte';
	import type { Snippet } from 'svelte';
	type ListItemProps = InstanceType<typeof ListItem>['$$prop_def'];

	// Omit + redeclare `title`: ListItemProps also pulls in the native HTML
	// `title` attribute (string), which collides with Konsta's `title` slot
	// prop and blocks passing a Snippet.
	interface Props extends Omit<ListItemProps, 'title'> {
		'data-testid'?: string;
		titleWrapClass?: string;
		title?: Snippet | string;
	}

	let { titleWrapClass = '', children, ...rest }: Props = $props();
</script>

<ListItem
	titleWrapClass={['title-truncated-wrap', titleWrapClass]
		.filter(Boolean)
		.join(' ')}
	innerClass="min-w-0"
	{...rest as ListItemProps}
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
		margin-inline-end: 8px;
	}
</style>
