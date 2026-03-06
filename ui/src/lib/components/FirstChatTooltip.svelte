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
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class={theme === 'ios'
			? 'fixed right-4 top-16 z-30'
			: 'fixed bottom-24 right-4 z-30'}
		onclick={dismiss}
		data-testid="first-chat-tooltip"
	>
		<div class="relative">
			{#if theme === 'ios'}
				<div class="absolute -top-2 right-5 h-0 w-0 border-x-8 border-b-8 border-x-transparent border-b-[#2c6bed]"></div>
			{/if}
			<div class="rounded-lg bg-[#2c6bed] px-4 py-2.5 text-sm font-medium text-white shadow-lg">
				{m.startFirstChatHere()}
			</div>
			{#if theme === 'material'}
				<div class="absolute -bottom-2 right-8 h-0 w-0 border-x-8 border-t-8 border-x-transparent border-t-[#2c6bed]"></div>
			{/if}
		</div>
	</div>
{/if}
