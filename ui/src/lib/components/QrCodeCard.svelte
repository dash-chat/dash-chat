<script lang="ts">
	import '@awesome.me/webawesome/dist/components/qr-code/qr-code.js';
	import { Card } from 'konsta/svelte';

	let {
		value,
		color,
		label,
	}: {
		value: string;
		color: string;
		label?: string | undefined;
	} = $props();

	const isWhite = $derived(color === '#ffffff');
	const qrColor = $derived(isWhite ? '#000000' : color);
</script>

<Card class="qr-card qr-code-card p-4 pb-0" style="background-color: {color}">
	<div class="column" style="align-items: center">
		<div
			class="column w-full p-3"
			style="align-items: center; justify-content: center; background-color: white; border-radius: 10px;"
		>
			<wa-qr-code
				{value}
				size="180"
				style="color: {qrColor}; --corner-color: {qrColor}"
			></wa-qr-code>
		</div>

		<div
			class="truncate max-w-40 pt-3.5 pb-2.5 in-[.k-ios]:pt-3 in-[.k-ios]:pb-2 text-sm font-medium {isWhite
				? 'text-black'
				: 'text-white'}"
		>
			{label ?? value}
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
