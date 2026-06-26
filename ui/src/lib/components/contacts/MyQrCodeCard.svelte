<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/qr-code/qr-code.js';
	import { Button, Card } from 'konsta/svelte';
	import { mdiContentCopy } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { writeText } from '$lib/utils/clipboard';
	import { showToast } from '$lib/utils/toasts';
	import { m } from '$lib/paraglide/messages.js';

	let {
		code,
		color,
	}: {
		code: string;
		color: string;
	} = $props();

	const isWhite = $derived(color === '#ffffff');
	const versionHint = $derived(
		code.includes('=') ? code.split('=')[1] : undefined,
	);

	async function copyLink() {
		await writeText(code);
		showToast(m.copiedCodeToClipboard());
	}
</script>

<Card class="qr-card my-code-card p-2.5 pb-2" style="background-color: {color}">
	<div class="column" style="align-items: center">
		<div
			class="column w-full p-3"
			style="align-items: center; justify-content: center; background-color: white; border-radius: 10px;"
		>
			<wa-qr-code value={code} size="180" fill={isWhite ? '#000000' : color}
			></wa-qr-code>
			{#if versionHint}
				<span
					class="mt-1"
					style="font-size: 11px; opacity: 0.5; font-family: monospace;"
					>{versionHint}</span
				>
			{/if}
		</div>

		<div class="py-1">
			<Button
				colors={{
					touchRipple: isWhite ? 'black' : 'white',
					textIos: isWhite ? 'text-black' : 'text-white',
					textMaterial: isWhite ? 'text-black' : 'text-white',
				}}
				clearIos
				clearMaterial
				small
				data-testid="add-contact-copy-btn"
				onClick={copyLink}
			>
				<wa-icon src={wrapPathInSvg(mdiContentCopy)}> </wa-icon>
				{code.slice(0, 15)}...
			</Button>
		</div>
	</div>
</Card>

<style>
	:global(.my-code-card) {
		align-self: center;
		width: fit-content;
		margin: 0 !important;
		transition: background-color 0.3s ease;
	}
</style>
