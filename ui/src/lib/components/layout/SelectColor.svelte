<script lang="ts">
	import { getContext, untrack } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';
	import { type SettingsStore } from 'dash-chat-stores';
	import { defaultQrColor } from '$lib/utils/qrcode';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Link,
		Button,
		useTheme,
	} from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import QrCodeCard from '$lib/components/QrCodeCard.svelte';

	let {
		code,
		qrColor,
		onClose,
	}: {
		code: string;
		qrColor: string;
		onClose: () => void;
	} = $props();

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');

	const qrColors = [
		defaultQrColor(),
		'#ffffff',
		'#8e8e93',
		'#a2845e',
		'#34c759',
		'#ff9500',
		'#ff2d55',
		'#af52de',
	];

	let qrColorIndex = $state(
		untrack(() => {
			const idx = qrColors.indexOf(qrColor);
			return idx === -1 ? 0 : idx;
		}),
	);

	const selectedColor = $derived(qrColors[qrColorIndex]);

	async function save() {
		try {
			await settingsStore.setQrColor(selectedColor);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error(e);
		}
		onClose();
	}
</script>

<Page style="display: flex; flex-direction: column">
	<Navbar
		centerTitle={theme === 'ios'}
		titleClass="opacity1"
		transparent={true}
		title={m.color()}
	>
		{#snippet left()}
			<NavbarBackLink data-testid="color-picker-back" onClick={onClose} />
		{/snippet}

		{#snippet right()}
			{#if isIos}
				<Link onClick={save} data-testid="color-picker-done">
					{m.done()}
				</Link>
			{/if}
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1; align-items: center;">
		<div
			class="column center-in-desktop gap-6 mx-4 mt-4"
			style="align-items: center; width: 100%; max-width: 400px;"
		>
			<QrCodeCard
				value={code}
				color={selectedColor}
				copyButtonTestId="color-picker-copy-btn"
				copiedMessage={m.copiedCodeToClipboard()}
			/>

			<div
				class="grid grid-cols-4 gap-4 justify-items-center px-4"
				data-testid="color-picker-grid"
			>
				{#each qrColors as color, i}
					<button
						class="w-14 h-14 rounded-full border-2 border-gray-400/30 cursor-pointer relative flex items-center justify-center transition-transform duration-150 p-0 hover:scale-[1.08] active:scale-95"
						style="background-color: {color};{qrColorIndex === i
							? ` box-shadow: 0 0 0 3px var(--k-bg-color, #fff), 0 0 0 5px ${color === '#ffffff' ? '#8e8e93' : color};`
							: ''}"
						onclick={() => (qrColorIndex = i)}
						data-testid="color-swatch-{i}"
						aria-label="Color {i + 1}"
					>
						{#if qrColorIndex === i}
							<div class="flex items-center justify-center">
								<svg
									viewBox="0 0 24 24"
									width="24"
									height="24"
									fill={color === '#ffffff' ? '#000' : '#fff'}
								>
									<path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
								</svg>
							</div>
						{/if}
					</button>
				{/each}
			</div>
		</div>
	</div>

	{#if !isIos}
		<Button
			rounded
			inline
			onClick={save}
			data-testid="color-picker-done"
			class="fixed-action-btn"
		>
			{m.done()}
		</Button>
	{/if}
</Page>
