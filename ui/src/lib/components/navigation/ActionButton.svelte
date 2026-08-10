<!--
	A full-width pill action, as used by the iOS confirmation sheets.
-->
<script lang="ts">
	import { Button } from 'konsta/svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		children: Snippet;
		onClick: () => void;
		/** Renders the label in red, for actions that destroy data. */
		destructive?: boolean;
		disabled?: boolean;
		'data-testid'?: string;
	};

	let {
		children,
		onClick,
		destructive = false,
		disabled = false,
		'data-testid': testId,
	}: Props = $props();
</script>

<Button
	large
	rounded
	tonal
	class="!font-normal"
	colors={{
		tonalBgIos:
			'bg-black/[0.06] active:bg-black/10 dark:bg-white/10 dark:active:bg-white/15',
		tonalTextIos: destructive ? 'text-red-500' : 'text-black dark:text-white',
	}}
	{onClick}
	{disabled}
	data-testid={testId}
>
	{@render children()}
</Button>
