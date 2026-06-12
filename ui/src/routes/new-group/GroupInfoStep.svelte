<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { BlockTitle, List, ListItem, useTheme } from 'konsta/svelte';
	import { mdiCamera } from '@mdi/js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import FormPage from '$lib/components/layout/FormPage.svelte';
	import SelectableContactList from '$lib/components/contacts/SelectableContactList.svelte';
	import SelectAvatar from '$lib/components/profiles/SelectAvatar.svelte';
	import type { Profile, VerifyingKey } from 'dash-chat-stores';

	interface Props {
		groupName: string;
		groupImage: string | undefined;
		selectedContacts: VerifyingKey[];
		resolvedContacts: [VerifyingKey, Profile][];
		onBack: () => void;
		onCreate: () => void;
	}

	let {
		groupName = $bindable(),
		groupImage = $bindable(),
		selectedContacts,
		resolvedContacts,
		onBack,
		onCreate,
	}: Props = $props();

	const theme = $derived(useTheme());
</script>

<FormPage
	title={m.nameThisGroup()}
	{onBack}
	backTestId="new-group-info-back"
	actionLabel={m.create()}
	onAction={onCreate}
	actionDisabled={groupName.trim() === ''}
	navbarTestId="new-group-info-navbar"
	actionTestId="new-group-create"
	constrainedWidth
>
	<div class="column" style="flex: 1">
		<List
			inset={isWideScreen.value || theme === 'ios'}
			strongIos
			nested={theme !== 'ios'}
		>
			<ListItem>
				{#snippet media()}
					<SelectAvatar
						bind:value={groupImage}
						size={56}
						placeholderIconPath={mdiCamera}
						placeholderLabel={m.groupAvatar()}
					/>
				{/snippet}
				{#snippet inner()}
					<input
						type="text"
						data-testid="new-group-name-input"
						placeholder={m.groupNameRequired()}
						bind:value={groupName}
						class="w-full bg-transparent border-none outline-none py-2 text-base"
					/>
				{/snippet}
			</ListItem>
		</List>

		<BlockTitle>{m.members()}</BlockTitle>

		<SelectableContactList
			contacts={resolvedContacts.filter(([key]) =>
				selectedContacts.includes(key),
			)}
			{selectedContacts}
			selectable={false}
			noDataMessage={m.youCanAddMembersLater()}
		/>
	</div>
</FormPage>
