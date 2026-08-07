<script lang="ts">
	import { Page } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';

	let { onContinue, onTerms }: { onContinue: () => void; onTerms: () => void } =
		$props();
</script>

<Page
	class="flex flex-col"
	style="background: linear-gradient(180deg, #6E7BFF 0.96%, #38369A 68.27%)"
>
	<div class="flex flex-1 flex-col items-center justify-center gap-20 px-8">
		<img src="/welcome-logo.svg" alt="" class="w-24" />
		<h1
			class="max-w-[16rem] text-center text-2xl font-bold leading-8 text-white"
			data-testid="welcome-title"
		>
			{m.welcomeTitle()}
		</h1>
	</div>

	<div
		class="flex flex-col items-center gap-7 px-11 pb-[calc(3rem+env(safe-area-inset-bottom))]"
	>
		<button
			type="button"
			class="text-sm text-white"
			onclick={onTerms}
			data-testid="welcome-terms-link"
		>
			{m.termsAndPrivacyPolicy()}
		</button>
		<button
			type="button"
			class="w-full rounded-[10px] bg-white py-4 text-base font-medium text-black"
			onclick={onContinue}
			data-testid="welcome-continue-btn"
		>
			{m.welcomeContinue()}
		</button>
	</div>
</Page>
