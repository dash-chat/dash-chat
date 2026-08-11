<script lang="ts">
	import { Button, Page } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { lightSystemBars } from '$lib/actions/light-system-bars';

	let { onContinue, onTerms }: { onContinue: () => void; onTerms: () => void } =
		$props();
</script>

<Page
	class="flex flex-col font-[inherit] [&_*]:font-[inherit]"
	style="background: linear-gradient(180deg, #6E7BFF 0.96%, #38369A 68.27%)"
>
	<div
		class="flex flex-1 flex-col items-center justify-center gap-16 px-8"
		use:lightSystemBars
	>
		<img src="/welcome-logo.svg" alt="" class="w-24" />
		<h1
			class="max-w-[16rem] text-center text-2xl font-bold leading-8 text-white"
			data-testid="welcome-title"
		>
			{m.welcomeTitle()}
		</h1>
	</div>

	<div
		class="flex w-full max-w-md flex-col gap-4 self-center px-11 pb-[calc(2.75rem+env(safe-area-inset-bottom))]"
	>
		<Button
			clear
			large
			class="font-normal"
			colors={{ textIos: 'text-white', textMaterial: 'text-white' }}
			onClick={onTerms}
			data-testid="welcome-terms-link"
		>
			{m.termsAndPrivacyPolicy()}
		</Button>
		<Button
			large
			class="!rounded-[10px]"
			colors={{
				fillBgIos: 'bg-white active:bg-white/90',
				fillBgMaterial: 'bg-white',
				fillTextIos: 'text-black',
				fillTextMaterial: 'text-black',
			}}
			onClick={onContinue}
			data-testid="welcome-continue-btn"
		>
			{m.welcomeContinue()}
		</Button>
	</div>
</Page>
