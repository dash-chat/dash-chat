<script lang="ts">
	import '@awesome.me/webawesome/dist/components/qr-code/qr-code.js';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiContentCopy } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { writeText } from '$lib/utils/clipboard';
	import { showToast } from '$lib/utils/toasts';
	import { type SettingsStore } from 'dash-chat-stores';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Link,
		Card,
		Button,
		useTheme,
	} from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';

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
		'#007aff',
		'#ffffff',
		'#8e8e93',
		'#a2845e',
		'#34c759',
		'#ff9500',
		'#ff2d55',
		'#af52de',
	];

	let qrColorIndex = $state(qrColors.indexOf(qrColor));
	if (qrColorIndex === -1) qrColorIndex = 0;

	const selectedColor = $derived(qrColors[qrColorIndex]);
	const isWhite = $derived(selectedColor === '#ffffff');

	async function save() {
		try {
			await settingsStore.setQrColor(selectedColor);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
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
			<Card
				class="qr-card p-2.5 pb-2"
				style="background-color: {selectedColor}"
			>
				<div class="column" style="align-items: center">
					<div
						class="column w-full p-3"
						style="align-items: center; justify-content: center; background-color: white; border-radius: 10px;"
					>
						<wa-qr-code
							value={code}
							size="180"
							fill={isWhite ? '#000000' : selectedColor}
						></wa-qr-code>
					</div>

					<div class="py-1">
						<Button
							colors={{
								touchRipple: isWhite ? 'black' : 'white',
								textIos: isWhite ? 'text-black' : 'text-white',
								textMaterial: isWhite ? 'text-black' : 'text-white',
							}}
							clear
							small
							data-testid="color-picker-copy-btn"
							onClick={async () => {
								await writeText(code);
								showToast(m.copiedCodeToClipboard());
							}}
						>
							<wa-icon src={wrapPathInSvg(mdiContentCopy)}> </wa-icon>

							{code.slice(0, 15)}...
						</Button>
					</div>
				</div>
			</Card>

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
