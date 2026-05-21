<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import type { PublicKey } from 'dash-chat-stores';
	import StepPage from './StepPage.svelte';

	interface Props {
		selectedContacts: PublicKey[];
		onNext: () => void;
	}

	let { selectedContacts = $bindable(), onNext }: Props = $props();
</script>

<StepPage
	title={m.newGroup()}
	backTestId="new-group-back"
	actionLabel={m.next()}
	onAction={onNext}
	actionLinkTestId="new-group-next-link"
	actionBtnTestId="new-group-next-btn"
>
	<p class="text-center mt-8 text-gray-500">
		Adding members not yet implemented
	</p>
</StepPage>
