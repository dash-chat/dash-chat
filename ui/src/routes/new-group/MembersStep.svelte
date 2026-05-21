<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { ContactsStore, PublicKey } from 'dash-chat-stores';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import ProfileAvatar from '$lib/components/profiles/ProfileAvatar.svelte';
	import {
		List,
		ListItem,
		Checkbox,
		BlockTitle,
		Preloader,
		useTheme,
	} from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import StepPage from './StepPage.svelte';

	interface Props {
		selectedContacts: PublicKey[];
		onNext: () => void;
	}

	let { selectedContacts = $bindable(), onNext }: Props = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const contacts = useReactivePromise(contactsStore.profilesForAllContacts);
	const theme = $derived(useTheme());
</script>

<StepPage
	title={m.newGroup()}
	backTestId="new-group-back"
	actionLabel={selectedContacts.length === 0 ? m.skip() : m.next()}
	onAction={onNext}
	actionLinkTestId="new-group-next-link"
	actionBtnTestId="new-group-next-btn"
>
	<div class="column" style="flex: 1">
		<div class="center-in-desktop">
			<BlockTitle>{m.contacts()}</BlockTitle>

			<List strongIos inset={isWideScreen.value || theme === 'ios'}>
				{#await $contacts}
					<div
						class="column"
						style="flex: 1; align-items: center; justify-content: center"
					>
						<Preloader />
					</div>
				{:then contacts}
					{#each contacts as [publicKey, profile]}
						<ListItem label title={profile.name}>
							{#snippet media()}
								<ProfileAvatar chatActorId={publicKey}></ProfileAvatar>
							{/snippet}

							{#snippet after()}
								<Checkbox
									checked={selectedContacts.includes(publicKey)}
									onChange={e => {
										const target = e.target as HTMLInputElement;
										if (target.checked) {
											selectedContacts = [...selectedContacts, publicKey];
										} else {
											selectedContacts = selectedContacts.filter(
												c => c !== publicKey,
											);
										}
									}}
								/>
							{/snippet}
						</ListItem>
					{:else}
						<ListItem title={m.noContactsYet()} />
					{/each}
				{/await}
			</List>
		</div>
	</div>
</StepPage>
