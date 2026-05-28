<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import StepPage from './StepPage.svelte';

	interface Props {
		onBack: () => void;
		onCreate: () => void;
	}

	let { onBack, onCreate }: Props = $props();
</script>

<StepPage
	title={m.groupName()}
	{onBack}
	backTestId="new-group-info-back"
	actionLabel={m.create()}
	onAction={onCreate}
	actionTestId="new-group-create"
>
	<p class="text-center mt-8 text-gray-500">
		Setting group info not yet implemented
	</p>
</StepPage>
