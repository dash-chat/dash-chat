<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import SelectAvatar from '$lib/components/profiles/SelectAvatar.svelte';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Button,
		Link,
		List,
		ListInput,
		useTheme,
	} from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import { isWideScreen } from '$lib/stores/screen.svelte';

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

<Page>
	<Navbar title={m.groupName()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink onClick={onBack} data-testid="new-group-info-back" />
		{/snippet}

		{#snippet right()}
			{#if isIos}
				<Link onClick={onCreate} data-testid="new-group-create-link">
					{m.create()}
				</Link>
			{/if}
		{/snippet}
	</Navbar>

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

	{#if !isIos}
		<Button
			onClick={onCreate}
			data-testid="new-group-create-btn"
			class="fixed-action-btn"
			rounded
		>
			{m.create()}
		</Button>
	{/if}
</Page>
