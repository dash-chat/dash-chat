<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import {
		BlockTitle,
		List,
		ListInput,
		ListItem,
		useTheme,
	} from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import StepPage from './StepPage.svelte';
	import SelectableContactList from '$lib/components/contacts/SelectableContactList.svelte';
	import type { Profile, PublicKey } from 'dash-chat-stores';

	interface Props {
		groupName: string;
		selectedContacts: PublicKey[];
		resolvedContacts: [PublicKey, Profile][];
		onBack: () => void;
		onCreate: () => void;
	}

	let {
		groupName = $bindable(),
		selectedContacts,
		resolvedContacts,
		onBack,
		onCreate,
	}: Props = $props();

	const theme = $derived(useTheme());
</script>

<StepPage
	title={m.nameThisGroup()}
	{onBack}
	backTestId="new-group-info-back"
	actionLabel={m.create()}
	onAction={onCreate}
	actionTestId="new-group-create"
>
	{#snippet subnavbar()}
		<div class="column gap-4">
			<List
				inset={isWideScreen.value || theme === 'ios'}
				strongIos
				nested={theme !== 'ios'}
			>
				<ListItem>Naming is not actually implemented yet.</ListItem>
				<!-- <ListInput
					type="text"
					bind:value={groupName}
					data-testid="new-group-name-input"
					outline
					class="plain"
					placeholder={m.name()}
				/> -->
			</List>
		</div>
	{/snippet}

	<div class="column" style="flex: 1">
		<BlockTitle>{m.members()}</BlockTitle>

		<SelectableContactList
			contacts={resolvedContacts.filter(([key]) =>
				selectedContacts.includes(key),
			)}
			{selectedContacts}
			selectable={false}
		/>
	</div>
</StepPage>
