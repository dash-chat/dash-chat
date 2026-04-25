<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { m } from '$lib/paraglide/messages';
	import { Button } from 'konsta/svelte';

	const isContactUsPage = $derived(
		page.url.pathname === '/settings/help/contact-us',
	);
</script>

<div
	class="test-banner relative z-30 px-4 py-1 text-center text-md font-bold"
	data-testid="test-banner"
>
	<span
		class="test-banner-text block pb-1"
		style="padding-top: env(safe-area-inset-top, 0px)">{m.testBanner()}</span
	>
	<Button
		onClick={() => {
			if (!isContactUsPage) goto('/settings/help/contact-us');
		}}
		disabled={isContactUsPage}
		small
		inline
		class="ml-2 normal-case"
		colors={{
			fillBgIos:
				'bg-white dark:bg-black active:bg-gray-100 dark:active:bg-gray-900',
			fillBgMaterial: 'bg-white dark:bg-black',
			fillTextIos: 'text-black dark:text-white',
			fillTextMaterial: 'text-black dark:text-white',
		}}>{m.feedback()}</Button
	>
</div>

<style>
	.test-banner {
		--banner-stripe-light: #eeeeee;
		--banner-stripe-dark: #dddddd;
		background: repeating-linear-gradient(
			45deg,
			var(--banner-stripe-light) 0 16px,
			var(--banner-stripe-dark) 16px 32px
		);
		border-color: var(--banner-stripe-dark);
		border-bottom-width: 2px;
		text-transform: uppercase;
		color: #333333;
		letter-spacing: 0.05em;
		word-spacing: 0.15em;
		margin-bottom: calc(0px - env(safe-area-inset-top, 0px));

		display: flex;
		flex-direction: row;
		align-items: flex-end;
		justify-content: space-between;
	}
	:global(.dark) .test-banner {
		--banner-stripe-light: #333333;
		--banner-stripe-dark: #222222;
		color: #cccccc;
	}
</style>
