<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { useTheme } from 'konsta/svelte';

	const SHOWN_KEY = 'first-chat-tooltip-shown';
	const theme = $derived(useTheme());

	// Only show on the very first app run — once shown, mark in localStorage so it never appears again
	const alreadyShown = localStorage.getItem(SHOWN_KEY) === '1';
	if (!alreadyShown) {
		localStorage.setItem(SHOWN_KEY, '1');
	}
	let dismissed = $state(false);

	function dismiss() {
		dismissed = true;
	}
</script>

{#if !alreadyShown && !dismissed}
	<div
		class="tooltip-bubble {theme === 'ios'
			? 'fixed end-4 top-16 z-30'
			: 'fixed bottom-24 end-4 z-30'}"
		role="status"
		tabindex="0"
		onclick={dismiss}
		onkeydown={(e) => { if (e.key === 'Escape') dismiss(); }}
		data-testid="first-chat-tooltip"
	>
		<div class="relative">
			{#if theme === 'ios'}
				<div class="arrow absolute -top-2 end-5 h-0 w-0 border-x-8 border-b-8 border-x-transparent"></div>
			{/if}
			<div class="tooltip-bg rounded-lg px-4 py-2.5 text-sm font-medium text-white shadow-lg">
				{m.startFirstChatHere()}
			</div>
			{#if theme === 'material'}
				<div class="arrow absolute -bottom-2 end-8 h-0 w-0 border-x-8 border-t-8 border-x-transparent"></div>
			{/if}
		</div>
	</div>
{/if}

<style>
	:root {
		--tooltip-bg: #2c6bed;
	}
	.tooltip-bg {
		background-color: var(--tooltip-bg);
	}
	.arrow {
		border-bottom-color: var(--tooltip-bg);
		border-top-color: var(--tooltip-bg);
	}
</style>
