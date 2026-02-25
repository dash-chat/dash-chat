<script lang="ts">
	import type { ContactsStore, Error } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Button,
		Link,
		List,
		ListInput,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Preloader,
		useTheme,
	} from 'konsta/svelte';
	import { showToast } from '$lib/utils/toasts';
	import { isWideScreen } from '$lib/stores/screen.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let name = $state<string>('');
	let surname = $state<string | undefined>(undefined);
	let avatar = $state<string | undefined>(undefined);
	let about = $state<string>('');

	const myProfile = useReactivePromise(contactsStore.myProfile);
	myProfile.subscribe((m) => {
		m.then((myProfile) => {
			if (!name) name = myProfile?.name || '';
			if (!surname) surname = myProfile?.surname;
			if (!avatar) avatar = myProfile?.avatar;
			if (about === '') about = myProfile?.about || '';
		});
	});

	const presets = [
		{ emoji: '\u{1F44B}', label: () => m.aboutSpeakFreely() },
		{ emoji: '\u{1F910}', label: () => m.aboutEncrypted() },
		{ emoji: '\u{1F64F}', label: () => m.aboutBeKind() },
		{ emoji: '\u2615', label: () => m.aboutCoffeeLover() },
		{ emoji: '\u{1F44D}', label: () => m.aboutFreeToChat() },
		{ emoji: '\u{1F6D1}', label: () => m.aboutTakingABreak() },
	];

	function selectPreset(emoji: string, label: string) {
		about = `${emoji} ${label}`;
	}

	function clearAbout() {
		about = '';
	}

	async function save() {
		try {
			await contactsStore.client.setProfile({
				name: name!,
				surname,
				avatar,
				about: about || undefined,
			});
			goto('/settings/profile');
		} catch (e) {
			console.error(e);
			const error = e as Error;
			switch (error.kind) {
				case 'AuthorOperation':
					showToast(m.errorSetProfile(), 'error');
					break;
				default:
					showToast(m.errorUnexpected(), 'unexpected');
			}
		}
	}

	const theme = $derived(useTheme());
	let originalAbout = $state<string | undefined>(undefined);
	myProfile.subscribe((m) => {
		m.then((myProfile) => {
			if (originalAbout === undefined) originalAbout = myProfile?.about || '';
		});
	});
	const hasChanges = $derived(about !== originalAbout);
</script>

<Page>
	{#await $myProfile}
		<div
			class="column"
			style="height: 100%; align-items: center; justify-content: center"
		>
			<Preloader />
		</div>
	{:then}
		<Navbar
			title={m.about()}
			titleClass="opacity1"
			transparent={true}
			rightClass={!hasChanges ? 'pointer-events-none opacity-50' : ''}
		>
			{#snippet left()}
				<NavbarBackLink
					onClick={() => goto('/settings/profile')}
					data-testid="edit-about-back"
				/>
			{/snippet}

			{#snippet right()}
				{#if theme === 'ios'}
					<Link onClick={save} data-testid="edit-about-save-link">
						{m.save()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>

		<div class="column">
			<List
				class="center-in-desktop"
				inset={isWideScreen.value || theme === 'ios'}
				strongIos
				nested={theme === 'material'}
			>
				<ListInput
					type="text"
					bind:value={about}
					placeholder={m.aboutPlaceholder()}
					data-testid="edit-about-input"
					clearButton={!!about}
					onClear={clearAbout}
				/>
			</List>

			<!-- Preset options -->
			<List
				class="center-in-desktop"
				inset={isWideScreen.value || theme === 'ios'}
				strongIos
				nested={theme === 'material'}
			>
				{#each presets as preset}
					<ListItem
						title={preset.label()}
						onClick={() => selectPreset(preset.emoji, preset.label())}
						data-testid="edit-about-preset"
					>
						{#snippet media()}
							<span class="text-2xl">{preset.emoji}</span>
						{/snippet}
					</ListItem>
				{/each}
			</List>
		</div>

		{#if theme === 'material'}
			<Button
				onClick={save}
				class="fixed-action-btn"
				rounded
				data-testid="edit-about-save-btn"
				disabled={!hasChanges}
			>
				{m.save()}
			</Button>
		{/if}
	{/await}
</Page>
