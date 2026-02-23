<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Dialog } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiAccountQuestion } from '@mdi/js';
	import { isWideScreen } from '$lib/stores/screen.svelte';

	interface Props {
		opened: boolean;
		onClose: () => void;
	}

	let { opened, onClose }: Props = $props();
</script>

{#snippet content()}
	<wa-icon
		src={wrapPathInSvg(mdiAccountQuestion)}
		style="font-size: 3rem"
	></wa-icon>

	<p class="text-center text-base">
		<strong>{m.profileNames()}</strong>
		{m.profileNamesExplanation()}
	</p>

	<div class="flex flex-col gap-4 w-full">
		<div class="flex items-start gap-3">
			<div class="w-1 self-stretch rounded bg-gray-400"></div>
			<span>{m.profileNamesNotVerifiedTip()}</span>
		</div>
		<div class="flex items-start gap-3">
			<div class="w-1 self-stretch rounded bg-gray-400"></div>
			<span>{m.profileNamesCautiousTip()}</span>
		</div>
		<div class="flex items-start gap-3">
			<div class="w-1 self-stretch rounded bg-gray-400"></div>
			<span>{m.profileNamesPersonalInfoTip()}</span>
		</div>
	</div>
{/snippet}

{#if isWideScreen.value}
	<Dialog {opened} onBackdropClick={onClose}>
		<div class="flex flex-col items-center gap-6">
			{@render content()}
		</div>
	</Dialog>
{:else}
	<Sheet class="pb-safe" {opened} onBackdropClick={onClose}>
		<div class="flex flex-col items-center gap-6 px-6 pb-6">
			<div class="sheet-handle"></div>
			{@render content()}
		</div>
	</Sheet>
{/if}
