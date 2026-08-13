<script lang="ts">
	import { List } from 'konsta/svelte';
	import { useTheme } from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import type { ComponentProps, Snippet } from 'svelte';

	type Props = {
		children: Snippet;
		class?: string;
		colors?: ComponentProps<List>['colors'];
		'data-testid'?: string;
	};

	let {
		children,
		class: className,
		colors,
		'data-testid': dataTestId,
	}: Props = $props();

	const theme = useTheme();
</script>

<List
	nested
	strongIos
	inset={isWideScreen.value || theme === 'ios'}
	{colors}
	data-testid={dataTestId}
	class={className}
>
	{@render children()}
</List>
