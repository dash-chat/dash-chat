<script lang="ts">
	import { goto } from '$app/navigation';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { type SettingsStore, type ColorScheme } from 'dash-chat-stores';
	import { showToast } from '$lib/utils/toasts';
	import { localesWithName } from '$lib/utils/localization';
	import {
		Dialog,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Radio,
		useTheme,
	} from 'konsta/svelte';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const colorScheme = useReactivePromise(settingsStore.colorScheme);
	const language = useReactivePromise(settingsStore.language);

	let showThemeDialog = $state(false);
	let showLanguageDialog = $state(false);

	async function selectScheme(scheme: ColorScheme) {
		try {
			await settingsStore.setColorScheme(scheme);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
		showThemeDialog = false;
	}

	function onThemeSelectChange(e: Event) {
		const target = e.target as HTMLSelectElement;
		selectScheme(target.value as ColorScheme);
	}

	async function selectLanguage(locale: string) {
		try {
			await settingsStore.setLanguage(locale);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
		showLanguageDialog = false;
	}

	function nameFromLocale(locale: string | null | undefined): string {
		if (!locale) return localesWithName[0].name;
		return localesWithName.find((l) => l.locale === locale)?.name ?? locale;
	}

	function schemeLabel(scheme: ColorScheme | undefined): string {
		if (scheme === 'light') return m.lightMode();
		if (scheme === 'dark') return m.darkMode();
		return m.systemDefault();
	}
</script>

<Page>
	<Navbar title={m.appearance()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="appearance-back"
				/>
			{/if}
		{/snippet}
	</Navbar>

	{#await $colorScheme then selectedScheme}
		{#await $language then currentLanguage}
			<div class="column" style="flex: 1">
				<div class="column center-in-desktop">
					<List strongIos inset={isWideScreen.value || theme === 'ios'}>
						<!-- Language -->
						<ListItem
							title={m.language()}
							after={nameFromLocale(currentLanguage)}
							link
							chevron={false}
							onClick={() => (showLanguageDialog = true)}
							data-testid="appearance-language"
						/>

						<!-- Theme: native select on desktop, dialog on mobile -->
						{#if isWideScreen.value}
							<ListItem
								title={m.theme()}
								data-testid="appearance-theme"
							>
								{#snippet after()}
									<select
										value={selectedScheme}
										onchange={onThemeSelectChange}
										class="appearance-auto border border-black/15 dark:border-white/20 rounded-md px-2 py-1.5 text-sm bg-transparent text-inherit cursor-pointer outline-none [&_option]:text-black [&_option]:bg-white dark:[&_option]:text-white dark:[&_option]:bg-[#1c1c1e]"
									>
										<option value="system">{m.systemDefault()}</option>
										<option value="light">{m.lightMode()}</option>
										<option value="dark">{m.darkMode()}</option>
									</select>
								{/snippet}
							</ListItem>
						{:else}
							<ListItem
								title={m.theme()}
								after={schemeLabel(selectedScheme)}
								link
								chevron={false}
								onClick={() => (showThemeDialog = true)}
								data-testid="appearance-theme"
							/>
						{/if}
					</List>
				</div>
			</div>

			<!-- Language dialog -->
			<Dialog
				opened={showLanguageDialog}
				onBackdropClick={() => (showLanguageDialog = false)}
			>
				{#snippet title()}
					{m.language()}
				{/snippet}
				<List nested class="-mx-4">
					{#each localesWithName as ln}
						<ListItem label title={ln.name} data-testid={`appearance-lang-${ln.locale}`}>
							{#snippet after()}
								<Radio
									component="div"
									value={ln.locale}
									checked={ln.locale === (currentLanguage ?? 'en')}
									onChange={() => selectLanguage(ln.locale)}
								/>
							{/snippet}
						</ListItem>
					{/each}
				</List>
			</Dialog>

			<!-- Theme dialog (mobile only) -->
			{#if !isWideScreen.value}
				<Dialog
					opened={showThemeDialog}
					onBackdropClick={() => (showThemeDialog = false)}
				>
					{#snippet title()}
						{m.theme()}
					{/snippet}
					<List nested class="-mx-4">
						<ListItem label title={m.systemDefault()} data-testid="appearance-theme-system">
							{#snippet after()}
								<Radio
									component="div"
									value="system"
									checked={selectedScheme === 'system'}
									onChange={() => selectScheme('system')}
								/>
							{/snippet}
						</ListItem>
						<ListItem label title={m.lightMode()} data-testid="appearance-theme-light">
							{#snippet after()}
								<Radio
									component="div"
									value="light"
									checked={selectedScheme === 'light'}
									onChange={() => selectScheme('light')}
								/>
							{/snippet}
						</ListItem>
						<ListItem label title={m.darkMode()} data-testid="appearance-theme-dark">
							{#snippet after()}
								<Radio
									component="div"
									value="dark"
									checked={selectedScheme === 'dark'}
									onChange={() => selectScheme('dark')}
								/>
							{/snippet}
						</ListItem>
					</List>
				</Dialog>
			{/if}
		{/await}
	{/await}
</Page>