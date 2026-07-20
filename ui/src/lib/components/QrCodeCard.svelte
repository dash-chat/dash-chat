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
		value,
		color,
		copyButtonTestId,
		label,
		copiedMessage = m.copiedToClipboard(),
	}: {
		value: string;
		color: string;
		copyButtonTestId: string;
		label?: string | undefined;
		copiedMessage?: string;
	} = $props();

	const isWhite = $derived(color === '#ffffff');

	async function copyLink() {
		await writeText(value);
		showToast(copiedMessage);
	}
</script>

<Card class="qr-card qr-code-card p-2.5 pb-2" style="background-color: {color}">
	<div class="column" style="align-items: center">
		<div
			class="column w-full p-3"
			style="align-items: center; justify-content: center; background-color: white; border-radius: 10px;"
		>
			<wa-qr-code {value} size="180" fill={isWhite ? '#000000' : color}
			></wa-qr-code>
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
				data-testid={copyButtonTestId}
				onClick={copyLink}
			>
				<wa-icon src={wrapPathInSvg(mdiContentCopy)}> </wa-icon>
				<div class="truncate max-w-40">
					{label ?? value}
				</div>
			</Button>
		</div>
	</div>
</Card>

<style>
	:global(.qr-code-card) {
		align-self: center;
		width: fit-content;
		margin: 0 !important;
		transition: background-color 0.3s ease;
	}
</style>
