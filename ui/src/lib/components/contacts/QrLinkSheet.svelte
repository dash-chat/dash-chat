<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet } from 'konsta/svelte';
	import { mdiContentCopy, mdiShareVariant } from '@mdi/js';
	import { shareText } from '$lib/utils/share';
	import { writeText } from '$lib/utils/clipboard';
	import { showToast } from '$lib/utils/toasts';
	import ActionList from '$lib/components/navigation/ActionList.svelte';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import BorderedBox from '$lib/components/BorderedBox.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';

	interface Props {
		opened: boolean;
		link: string;
		onClose: () => void;
	}

	let { opened, link, onClose }: Props = $props();

	async function copyLink() {
		await writeText(link);
		showToast(m.copiedLinkToClipboard());
	}

	async function share() {
		try {
			await shareText(link);
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
</script>

<Sheet class="pb-safe" {opened} onBackdropClick={onClose}>
	<div data-testid="qr-link-sheet">
		<div class="flex flex-col items-center gap-6 px-6">
			<SheetHandle />

			<p class="text-center text-sm quiet">{m.shareLinkWarning()}</p>

			<BorderedBox class="w-full" data-testid="qr-link-sheet-link">
				<span class="break-all text-start text-sm">{link}</span>
			</BorderedBox>
		</div>

		<ActionList class="py-4">
			<ListAction
				title={m.copyLink()}
				icon={mdiContentCopy}
				onClick={() => void copyLink()}
				data-testid="qr-link-sheet-copy"
			/>
			<ListAction
				title={m.share()}
				icon={mdiShareVariant}
				onClick={() => void share()}
				data-testid="qr-link-sheet-share"
			/>
		</ActionList>
	</div>
</Sheet>
