<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import {
		mdiLinkVariant,
		mdiShareVariant,
		mdiTrayArrowDown,
		mdiPalette,
	} from '@mdi/js';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';

	type ActionHandler = () => void | Promise<void>;

	let {
		isMobile,
		onCopyLink,
		onShare,
		onSave,
		onOpenColorPicker,
	}: {
		isMobile: boolean;
		onCopyLink: ActionHandler;
		onShare: ActionHandler;
		onSave: ActionHandler;
		onOpenColorPicker: ActionHandler;
	} = $props();
</script>

<!-- Action buttons: Link, Share, Save, Color -->
<div class="row gap-4" style="justify-content: center;">
	<div class="column" style="display: none; align-items: center; gap: 8px;">
		<LabelledIconButton
			label={m.link()}
			icon={mdiLinkVariant}
			onClick={onCopyLink}
			testId="add-contact-link-btn"
		/>
	</div>

	{#if isMobile}
		<LabelledIconButton
			label={m.share()}
			icon={mdiShareVariant}
			onClick={onShare}
			testId="add-contact-share-btn"
		/>
	{:else}
		<LabelledIconButton
			label={m.save()}
			icon={mdiTrayArrowDown}
			onClick={onSave}
			testId="add-contact-save-btn"
		/>
	{/if}

	<LabelledIconButton
		label={m.color()}
		icon={mdiPalette}
		onClick={onOpenColorPicker}
		testId="add-contact-color-btn"
	/>
</div>
