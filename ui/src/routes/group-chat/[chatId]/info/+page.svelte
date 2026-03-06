<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ContactsStore, ChatsStore, PublicKey } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiAccountGroup,
		mdiClose,
		mdiDelete,
		mdiExport,
		mdiKeyVariant,
		mdiPencil,
		mdiPlusCircle,
	} from '@mdi/js';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Button,
		Card,
		Link,
		List,
		ListItem,
		Chip,
		Dialog,
		DialogButton,
		Sheet,
		ActionsButton,
		BlockTitle,
		useTheme,
	} from 'konsta/svelte';
	import Layout from '../../../+layout.svelte';

	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { isMobile } from '$lib/utils/environment';
	import { page } from '$app/state';
	let chatId = page.params.chatId!;

	const chatsStore: ChatsStore = getContext('chats-store');
	const groupChatStore = chatsStore.groupChats(chatId);

	const info = useReactivePromise(groupChatStore.info);
	const members = useReactivePromise(groupChatStore.allMembers);
	const me = useReactivePromise(groupChatStore.me);

	let sheetOpenFor = $state<string | null>(null);
	let dialogType = $state<
		'demote' | 'promote' | 'remove' | 'leave' | 'delete' | null
	>(null);
	let dialogActorId = $state<string | null>(null);
	let loading = $state(false);
	const theme = $derived(useTheme());

	async function handleDemote(actorId: string) {
		loading = true;
		try {
			await groupChatStore.client.demoteFromAdministrator(chatId, actorId);
			dialogType = null;
			dialogActorId = null;
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}

	async function handlePromote(actorId: string) {
		loading = true;
		try {
			await groupChatStore.client.promoteToAdministrator(chatId, actorId);
			dialogType = null;
			dialogActorId = null;
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}

	async function handleRemove(actorId: string) {
		loading = true;
		try {
			await groupChatStore.client.removeMember(chatId, actorId);
			dialogType = null;
			dialogActorId = null;
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}

	async function handleLeaveGroup() {
		loading = true;
		try {
			await groupChatStore.client.leaveGroup();
			dialogType = null;
			goto('/');
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}

	async function handleDeleteGroup() {
		loading = true;
		try {
			await groupChatStore.client.deleteGroup();
			dialogType = null;
			goto('/');
		} catch (e) {
			console.error(e);
		}
		loading = false;
	}
</script>

<Page>
	{#await $info then info}
		<Navbar transparent={true}>
			{#snippet left()}
				<NavbarBackLink onClick={() => goto(`/group-chat/${chatId}`)} />
			{/snippet}

			{#snippet title()}
				<div
					class="gap-2"
					style="display: flex; justify-content: start; align-items: center; flex: 1"
				>
					<wa-avatar
						image={info.avatar}
						initials={info.name.slice(0, 2)}
						style="--size: 2.5rem"
					>
					</wa-avatar>
					<span>{info.name}</span>
				</div>
			{/snippet}

			{#snippet right()}
				<Link
					href={`/group-chat/${chatId}/info/edit`}
					iconOnly={theme === 'material'}
				>
					{#if theme === 'material'}
						<wa-icon src={wrapPathInSvg(mdiPencil)}> </wa-icon>
					{:else}
						{m.edit()}
					{/if}
				</Link>
			{/snippet}
		</Navbar>

		{#await $me then me}
			<div class="column" style="flex: 1">
				<div class="column center-in-desktop gap-8 p-2">
					<div class="column" style="align-items: center; gap: 1rem">
						<wa-avatar image={info.avatar} style="--size: 5rem">
							<wa-icon src={wrapPathInSvg(mdiAccountGroup)}> </wa-icon>
						</wa-avatar>

						<span class={`${isMobile ? 'text-xl' : 'text-lg'} font-semibold`}>{info.name}</span>

						<span class="quiet">{info.description}</span>
					</div>

					{#await $members then members}
						<BlockTitle>
							{m.membersCount({
								count: Object.keys(members).length,
							})}</BlockTitle
						>
						<List nested strongIos inset={isWideScreen.value || theme === 'ios'}>
							{#if me.admin}
								<ListItem
									link
									chevron={false}
									linkProps={{ href: `/group-chat/${chatId}/info/add-members` }}
									title={m.addMembers()}
								>
									{#snippet media()}
										<wa-icon
											style="font-size: 2rem;"
											src={wrapPathInSvg(mdiPlusCircle)}
										></wa-icon>
									{/snippet}
								</ListItem>
							{/if}

							{#each Object.entries(members) as [actorId, member]}
								<ListItem
									link
									chevron={false}
									title={member.profile?.name}
									onclick={() => (sheetOpenFor = actorId)}
								>
									{#snippet media()}
										<wa-avatar
											image={member.profile?.avatar}
											initials={member.profile?.name.slice(0, 2)}
										></wa-avatar>
									{/snippet}

									{#snippet after()}
										{#if member.admin}
											<Chip>{m.administrator()}</Chip>
										{/if}
									{/snippet}
								</ListItem>

								<Sheet
									class="pb-safe"
									opened={sheetOpenFor === actorId}
									onBackdropClick={() => (sheetOpenFor = null)}
								>
									<div
										class="flex-col gap-4 py-4"
										style="display: flex; align-items: center;"
									>
										<wa-avatar
											image={member.profile?.avatar}
											initials={member.profile?.name.slice(0, 2)}
										></wa-avatar>
										<span class="font-semibold">{member.profile?.name}</span>
									</div>

									<List nested strongIos inset={isWideScreen.value || theme === 'ios'} class="mb-2">
										{#if me.admin}
											{#if member.admin}
												<ListItem
													link
													chevron={false}
													title={m.demoteFromAdministrator()}
													onClick={() => {
														dialogType = 'demote';
														dialogActorId = actorId;
														sheetOpenFor = null;
													}}
												>
													{#snippet media()}
														<wa-icon src={wrapPathInSvg(mdiKeyVariant)}
														></wa-icon>
													{/snippet}
												</ListItem>
											{:else}
												<ListItem
													link
													chevron={false}
													title={m.promoteToAdministrator()}
													onClick={() => {
														dialogType = 'promote';
														dialogActorId = actorId;
														sheetOpenFor = null;
													}}
												>
													{#snippet media()}
														<wa-icon src={wrapPathInSvg(mdiKeyVariant)}
														></wa-icon>
													{/snippet}
												</ListItem>
											{/if}

											<ListItem
												link
												chevron={false}
												title={m.removeMember()}
												onClick={() => {
													dialogType = 'remove';
													dialogActorId = actorId;
													sheetOpenFor = null;
												}}
											>
												{#snippet media()}
													<wa-icon src={wrapPathInSvg(mdiDelete)}></wa-icon>
												{/snippet}
											</ListItem>
										{/if}
									</List>
								</Sheet>
							{/each}
						</List>

						<List nested strongIos inset={isWideScreen.value || theme === 'ios'} class="z-1">
							<ListItem
								title={m.leaveGroup()}
								link
								chevron={false}
								onClick={() => (dialogType = 'leave')}
								colors={{
									primaryTextIos: 'text-red-500',
									primaryTextMaterial: 'text-red-600',
								}}
							>
								{#snippet media()}
									<wa-icon class="big" src={wrapPathInSvg(mdiExport)}></wa-icon>
								{/snippet}
							</ListItem>

							<ListItem
								title={m.deleteGroup()}
								link
								chevron={false}
								onClick={() => (dialogType = 'delete')}
								colors={{
									primaryTextIos: 'text-red-500',
									primaryTextMaterial: 'text-red-600',
								}}
							>
								{#snippet media()}
									<wa-icon class="big" src={wrapPathInSvg(mdiClose)}></wa-icon>
								{/snippet}
							</ListItem>
						</List>
					{/await}
				</div>
			</div>
		{/await}

		<!-- Dialogs -->
		<Dialog
			opened={dialogType === 'demote' && dialogActorId !== null}
			onBackdropClick={() => {
				dialogType = null;
				dialogActorId = null;
			}}
		>
			{#snippet title()}
				{m.demoteFromAdministrator()}
			{/snippet}
			<span>{m.areYouSureDemote()}</span>
			{#snippet buttons()}
				<DialogButton
					onClick={() => {
						dialogType = null;
						dialogActorId = null;
					}}
				>
					{m.cancel()}
				</DialogButton>
				<DialogButton
					strong
					onClick={() => dialogActorId && handleDemote(dialogActorId)}
					disabled={loading}
				>
					{loading ? '...' : m.demote()}
				</DialogButton>
			{/snippet}
		</Dialog>

		<Dialog
			opened={dialogType === 'promote' && dialogActorId !== null}
			onBackdropClick={() => {
				dialogType = null;
				dialogActorId = null;
			}}
		>
			{#snippet title()}
				{m.promoteToAdministrator()}
			{/snippet}
			<span>{m.areYouSurePromote()}</span>
			{#snippet buttons()}
				<DialogButton
					onClick={() => {
						dialogType = null;
						dialogActorId = null;
					}}
				>
					{m.cancel()}
				</DialogButton>
				<DialogButton
					onClick={() => dialogActorId && handlePromote(dialogActorId)}
					disabled={loading}
					strong
				>
					{loading ? '...' : m.promote()}
				</DialogButton>
			{/snippet}
		</Dialog>

		<Dialog
			opened={dialogType === 'remove' && dialogActorId !== null}
			onBackdropClick={() => {
				dialogType = null;
				dialogActorId = null;
			}}
		>
			{#snippet title()}
				{m.removeMember()}
			{/snippet}
			<span>{m.areYouSureRemoveMember()}</span>
			{#snippet buttons()}
				<DialogButton
					onClick={() => {
						dialogType = null;
						dialogActorId = null;
					}}
				>
					{m.cancel()}
				</DialogButton>
				<DialogButton
					strong
					onClick={() => dialogActorId && handleRemove(dialogActorId)}
					disabled={loading}
				>
					{loading ? '...' : m.remove()}
				</DialogButton>
			{/snippet}
		</Dialog>

		<Dialog
			opened={dialogType === 'leave'}
			onBackdropClick={() => (dialogType = null)}
		>
			{#snippet title()}
				{m.leaveGroup()}
			{/snippet}
			<span>{m.areYouSureLeaveGroup()}</span>
			{#snippet buttons()}
				<DialogButton onClick={() => (dialogType = null)}
					>{m.cancel()}</DialogButton
				>
				<DialogButton strong onClick={handleLeaveGroup} disabled={loading}>
					{loading ? '...' : m.leave()}
				</DialogButton>
			{/snippet}
		</Dialog>

		<Dialog
			opened={dialogType === 'delete'}
			onBackdropClick={() => (dialogType = null)}
		>
			{#snippet title()}
				{m.deleteGroup()}
			{/snippet}
			<span>{m.areYouSureDeleteGroup()}</span>
			{#snippet buttons()}
				<DialogButton onClick={() => (dialogType = null)}
					>{m.cancel()}</DialogButton
				>
				<DialogButton strong onClick={handleDeleteGroup} disabled={loading}>
					{loading ? '...' : m.delete()}
				</DialogButton>
			{/snippet}
		</Dialog>
	{/await}
</Page>

<style>
	wa-avatar {
		--size: 32px;
	}
	wa-icon {
		width: 32px;
	}
</style>
