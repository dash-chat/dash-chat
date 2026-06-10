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
	import { isIos } from '$lib/utils/environment';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import FormInput from '$lib/components/form/FormInput.svelte';
	import Form from '$lib/components/form/Form.svelte';
	import Container from '$lib/components/layout_helpers/Container.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let name = $state<string>('');
	let surname = $state<string | undefined>(undefined);
	let avatar = $state<string | undefined>(undefined);
	let about = $state<string>('');
	let originalAbout = $state<string | undefined>(undefined);

	const myProfile = useReactivePromise(contactsStore.myProfile);
	let initialized = false;
	$effect(() => {
		$myProfile.then(profile => {
			if (!initialized) {
				initialized = true;
				name = profile?.name || '';
				surname = profile?.surname;
				avatar = profile?.avatar;
				about = profile?.about || '';
				originalAbout = profile?.about || '';
			}
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
					showToast(m.errorUnexpected(), 'unexpected', e);
			}
		}
	}

	const theme = $derived(useTheme());
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
			rightClass={!hasChanges ? 'ios-right-disabled' : ''}
		>
			{#snippet left()}
				<NavbarBackLink
					onClick={() => goto('/settings/profile')}
					data-testid="edit-about-back"
				/>
			{/snippet}

			{#snippet right()}
				{#if isIos}
					<Link onClick={save} data-testid="edit-about-save-btn">
						{m.save()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>

		<Container>
			<Form>
				<FormInput
					type="text"
					bind:value={about}
					placeholder={m.aboutPlaceholder()}
					data-testid="edit-about-input"
					clearButton={!!about}
					onClear={clearAbout}
				/>
			</Form>

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
		</Container>

		{#if !isIos}
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
