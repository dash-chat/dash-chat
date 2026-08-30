<script lang="ts">
	import { Button, Preloader } from 'konsta/svelte';
	import type { Snippet } from 'svelte';
	import { renderAboveKeyboard } from '$lib/utils/virtual-keyboard/render-above-keyboard';

	interface Props {
		onClick: () => void;
		disabled?: boolean;
		loading?: boolean;
		tonal?: boolean;
		testId?: string;
		children: Snippet;
	}

	let {
		onClick,
		disabled = false,
		loading = false,
		tonal = false,
		testId,
		children,
	}: Props = $props();
</script>

<!-- Viewport-fixed, so a padded ancestor can't lift it above the keyboard. -->
<div
	class="fixed end-4 bottom-[calc(var(--keyboard-safe-bottom,0px)+1rem)]"
	use:renderAboveKeyboard
>
	<Button
		rounded
		inline
		{tonal}
		disabled={disabled || loading}
		{onClick}
		data-testid={testId}
	>
		{@render children()}
		{#if loading}
			<Preloader
				class="ms-2 h-4 w-4"
				colors={{ iconIos: 'text-current', iconMaterial: 'text-current' }}
			/>
		{/if}
	</Button>
</div>
