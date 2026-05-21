<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import SelectAvatar from '$lib/components/profiles/SelectAvatar.svelte';
	import { List, ListInput, useTheme } from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import StepPage from './StepPage.svelte';

	interface Props {
		groupName: string;
		groupAvatar: string | undefined;
		onBack: () => void;
		onCreate: () => void;
	}

	let {
		groupName = $bindable(),
		groupAvatar = $bindable(),
		onBack,
		onCreate,
	}: Props = $props();

	let avatarBinding = $state(groupAvatar ?? '');
	$effect(() => {
		groupAvatar = avatarBinding || undefined;
	});

	const theme = $derived(useTheme());
</script>

<StepPage
	title={m.groupName()}
	{onBack}
	backTestId="new-group-info-back"
	actionLabel={m.create()}
	onAction={onCreate}
	actionLinkTestId="new-group-create-link"
	actionBtnTestId="new-group-create-btn"
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
				>
					{#snippet media()}
						<SelectAvatar bind:value={avatarBinding}></SelectAvatar>
					{/snippet}
				</ListInput>
			</List>
		</div>
	</div>
</StepPage>
