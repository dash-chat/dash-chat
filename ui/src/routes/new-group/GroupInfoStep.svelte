<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { List, ListInput, useTheme } from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import StepPage from './StepPage.svelte';

	interface Props {
		groupName: string;
		onBack: () => void;
		onCreate: () => void;
	}

	let { groupName = $bindable(), onBack, onCreate }: Props = $props();

	const theme = $derived(useTheme());
</script>

<StepPage
	title={m.groupName()}
	{onBack}
	backTestId="new-group-info-back"
	actionLabel={m.create()}
	onAction={onCreate}
	actionTestId="new-group-create"
>
	<div class="column" style="flex: 1">
		<div class="center-in-desktop m-1">
			<List
				inset={isWideScreen.value || theme === 'ios'}
				strongIos
				nested={theme !== 'ios'}
			>
				<ListInput
					type="text"
					bind:value={groupName}
					data-testid="new-group-name-input"
					outline
					class="plain"
					placeholder={m.name()}
				/>
			</List>
		</div>
	</div>
</StepPage>
