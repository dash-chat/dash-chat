<script lang="ts">
	import '@awesome.me/webawesome/dist/components/carousel/carousel.js';
	import '@awesome.me/webawesome/dist/components/carousel-item/carousel-item.js';
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Button } from 'konsta/svelte';

	interface Props {
		opened: boolean;
		onClose: () => void;
	}

	let { opened, onClose }: Props = $props();

	let carousel: HTMLElement | undefined = $state();
	let currentTip = $state(0);

	const tips = [
		{
			emoji: '\u{1F46E}\u{26A0}\u{FE0F}',
			title: () => m.securityTipFakeNamesTitle(),
			description: () => m.securityTipFakeNamesDescription(),
		},
		{
			emoji: '\u{1F4B0}\u{1F680}\u{1F44E}',
			title: () => m.securityTipCryptoScamsTitle(),
			description: () => m.securityTipCryptoScamsDescription(),
		},
		{
			emoji: '\u{1F44B}',
			title: () => m.securityTipVagueMessagesTitle(),
			description: () => m.securityTipVagueMessagesDescription(),
		},
		{
			emoji: '\u{1F517}\u{1F6A8}',
			title: () => m.securityTipLinksTitle(),
			description: () => m.securityTipLinksDescription(),
		},
		{
			emoji: '\u{1F4E6}\u{1F4B0}\u{2753}\u{1F914}',
			title: () => m.securityTipFakeBusinessesTitle(),
			description: () => m.securityTipFakeBusinessesDescription(),
		},
	];

	$effect(() => {
		if (!carousel) return;
		const node = carousel;
		customElements.whenDefined('wa-carousel').then(() => {
			const shadow = node.shadowRoot;
			if (!shadow || shadow.querySelector('#safety-tips-fix')) return;
			const style = document.createElement('style');
			style.id = 'safety-tips-fix';
			style.textContent = `
				:host {
					--aspect-ratio: auto !important;
				}
				.carousel {
					min-height: unset !important;
					grid-template:
						". slides ." max-content
						". pagination ." min-content
						/ min-content 1fr min-content !important;
				}
				.slides {
					aspect-ratio: unset !important;
					height: auto !important;
				}
				.slides-horizontal {
					grid-auto-rows: max-content !important;
				}
				.pagination-item-active {
					background-color: black !important;
					transform: none !important;
				}
				@media (prefers-color-scheme: dark) {
					.pagination-item-active {
						background-color: white !important;
					}
				}
			`;
			shadow.appendChild(style);
		});
	});

	function onSlideChange(e: CustomEvent) {
		currentTip = e.detail.index;
	}

	function nextTip() {
		(carousel as any)?.next();
	}

	function previousTip() {
		(carousel as any)?.previous();
	}

	function handleClose() {
		currentTip = 0;
		(carousel as any)?.goToSlide(0, 'auto');
		onClose();
	}
</script>

<Sheet
	class="pb-safe"
	opened={opened}
	onBackdropClick={handleClose}
>
	<div class="flex flex-col items-center px-6 pb-4 gap-4">
		<div class="sheet-handle"></div>

		<h2 class="mt-2 text-2xl font-bold">{m.securityTips()}</h2>
		<p class=" text-center text-sm text-gray-500">
			{m.securityTipsSubtitle()}
		</p>

		<!-- svelte-ignore element_invalid_self_closing_tag -->
		<wa-carousel
			bind:this={carousel}
			pagination
			class="w-full"
			onwa-slide-change={onSlideChange}
		>
			{#each tips as tip}
				<wa-carousel-item>
					<div class="mx-2 flex flex-col items-center rounded-2xl bg-white p-4 dark:bg-black">
						<div class="mb-4 w-full rounded-2xl bg-gray-50 p-6 dark:bg-gray-800">
							<div class="flex min-h-24 items-center justify-center text-5xl">
								{tip.emoji}
							</div>
						</div>
						<h3 class="mb-2 text-center text-lg font-bold">
							{tip.title()}
						</h3>
						<p class="text-center text-sm text-gray-500">
							{tip.description()}
						</p>
					</div>
				</wa-carousel-item>
			{/each}
		</wa-carousel>

		<div class="flex w-full items-center justify-between">
			<Button
				clear
				rounded
				onClick={previousTip}
				disabled={currentTip === 0}
				class="w-auto"
			>
				{m.securityTipsPrevious()}
			</Button>
			<Button
				tonal
				rounded
				onClick={currentTip === tips.length - 1 ? handleClose : nextTip}
				class="w-auto"
			>
				{currentTip === tips.length - 1 ? m.done() : m.securityTipsNext()}
			</Button>
		</div>
	</div>
</Sheet>
