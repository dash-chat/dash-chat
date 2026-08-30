<script lang="ts">
	import type { ContactWithProfile, VerifyingKey } from 'dash-chat-stores';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import { List, ListItem, Checkbox, Preloader, useTheme } from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';

	interface Props {
		contacts: ContactWithProfile[];
		selectedContacts: VerifyingKey[];
		loading?: boolean;
		noDataMessage?: string;
		selectable?: boolean;
	}

	let {
		contacts,
		selectedContacts = $bindable(),
		loading = false,
		noDataMessage,
		selectable = true,
	}: Props = $props();

	const theme = $derived(useTheme());
</script>

<div class="column min-h-40 flex-1 items-stretch justify-start">
	{#if loading}
		<div class="column flex-1 items-center justify-center">
			<Preloader />
		</div>
	{:else}
		<List strongIos inset={isWideScreen.value || theme === 'ios'}>
			{#each contacts as { contact, profile }}
				{@const verifyingKey = contact.agentId}
				<ListItem
					label
					title={profile.name}
					data-testid="selectable-contact-item"
					data-contact-name={profile.name}
				>
					{#snippet media()}
						<Avatar
							image={profile.avatar}
							initials={profile.name.slice(0, 2)}
						/>
					{/snippet}

					{#snippet after()}
						{#if selectable}
							<Checkbox
								checked={selectedContacts.includes(verifyingKey)}
								onChange={e => {
									const target = e.target as HTMLInputElement;
									if (target.checked) {
										selectedContacts = [...selectedContacts, verifyingKey];
									} else {
										selectedContacts = selectedContacts.filter(
											c => c !== verifyingKey,
										);
									}
								}}
							/>
						{/if}
					{/snippet}
				</ListItem>
			{/each}
			{#if contacts.length === 0 && noDataMessage}
				<ListItem
					title={noDataMessage}
					data-testid="selectable-contacts-empty"
				/>
			{/if}
		</List>
	{/if}
</div>
