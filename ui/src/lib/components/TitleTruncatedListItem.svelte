<script lang="ts">
	import { ListItem } from 'konsta/svelte';
	type ListItemProps = InstanceType<typeof ListItem>['$$prop_def'];

	interface Props extends ListItemProps {
		'data-testid'?: string;
		titleWrapClass?: string;
	}

	let { titleWrapClass = '', children, ...rest }: Props = $props();
</script>

<ListItem
	titleWrapClass={['title-truncated-wrap', titleWrapClass]
		.filter(Boolean)
		.join(' ')}
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
		margin-inline-end: 8px;
	}
</style>
