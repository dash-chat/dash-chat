<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, useTheme } from 'konsta/svelte';
	import { mdiContentCopy } from '@mdi/js';
	import { mdiShare } from '$lib/utils/icon';
	import { shareText } from '$lib/utils/share';
	import { copyLinkToClipboard } from '$lib/utils/clipboard';
	import { showToast } from '$lib/utils/toasts';
	import ActionList from '$lib/components/navigation/ActionList.svelte';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import BorderedBox from '$lib/components/BorderedBox.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import { isIos } from '$lib/utils/environment';

	interface Props {
		opened: boolean;
		link: string;
		onClose: () => void;
	}

	let { opened, link, onClose }: Props = $props();

	async function share() {
		try {
			await shareText(link);
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
	const theme = $derived(useTheme());
</script>

<Sheet
	class="pb-safe {theme === 'ios' ? 'bg-page-surface' : ''}"
	{opened}
	onBackdropClick={onClose}
>
	<div class="flex-col" data-testid="qr-link-sheet">
		<div class="flex flex-col items-center gap-6 px-4">
			<SheetHandle />

			<p class="text-center text-sm quiet">{m.shareLinkWarning()}</p>

			{#if isIos}
				<div
					class="rounded-2xl bg-ios-light-surface-1 px-4 py-3 dark:bg-ios-dark-surface-1"
					data-testid="qr-link-sheet-link"
				>
					<span class="break-all text-start text-sm">{link}</span>
				</div>
			{:else}
				<BorderedBox class="w-full" data-testid="qr-link-sheet-link">
					<span class="break-all text-start text-sm">{link}</span>
				</BorderedBox>
			{/if}
		</div>

		<div class="pt-4 pb-8">
			<ActionList>
				<ListAction
					title={m.copyLink()}
					icon={mdiContentCopy}
					onClick={() => void copyLinkToClipboard(link)}
					data-testid="qr-link-sheet-copy"
				/>
				<ListAction
					title={m.share()}
					icon={mdiShare}
					onClick={() => void share()}
					data-testid="qr-link-sheet-share"
				/>
			</ActionList>
		</div>
	</div>
</Sheet>
