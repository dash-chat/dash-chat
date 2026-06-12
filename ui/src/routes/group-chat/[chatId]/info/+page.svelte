<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatsStore } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiAccountGroup,
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
		ListItem,
		Chip,
		Sheet,
		BlockTitle,
		useTheme,
		Link,
	} from 'konsta/svelte';

	import { page } from '$app/state';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import ActionList from '$lib/components/navigation/ActionList.svelte';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
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
	const theme = $derived(useTheme());

	// async function handleDemote(actorId: string) {
	// 	loading = true;
	// 	try {
	// 		await groupChatStore.client.demoteFromAdministrator(chatId, actorId);
	// 		dialogType = null;
	// 		dialogActorId = null;
	// 	} catch (e) {
	// 		console.error(e);
	// 	}
	// 	loading = false;
	// }

	// async function handlePromote(actorId: string) {
	// 	loading = true;
	// 	try {
	// 		await groupChatStore.client.promoteToAdministrator(chatId, actorId);
	// 		dialogType = null;
	// 		dialogActorId = null;
	// 	} catch (e) {
	// 		console.error(e);
	// 	}
	// 	loading = false;
	// }

	// async function handleRemove(actorId: string) {
	// 	loading = true;
	// 	try {
	// 		await groupChatStore.client.removeMember(chatId, actorId);
	// 		dialogType = null;
	// 		dialogActorId = null;
	// 	} catch (e) {
	// 		console.error(e);
	// 	}
	// 	loading = false;
	// }

	async function handleLeaveGroup() {
		try {
			await groupChatStore.client.leaveGroup(chatId);
			goto('/');
			dialogType = null;
			return { success: true as const };
		} catch (e) {
			console.error(e);
			return {
				success: false as const,
				error: m.errorLeavingGroup(),
			};
		}
	}

	// async function handleDeleteGroup() {
	// 	loading = true;
	// 	try {
	// 		await groupChatStore.client.deleteGroup();
	// 		dialogType = null;
	// 		goto('/');
	// 	} catch (e) {
	// 		console.error(e);
	// 	}
	// 	loading = false;
	// }
</script>

<Page>
	{#await $info then info}
		<Navbar transparent={true}>
			{#snippet left()}
				<NavbarBackLink
					data-testid="group-info-back"
					onClick={() => goto(`/group-chat/${chatId}`)}
				/>
			{/snippet}

			{#snippet title()}
				<div
					class="gap-2"
					style="display: flex; justify-content: start; align-items: center; flex: 1"
				>
					<Avatar
						image={info.image}
						initials={info.name.slice(0, 2)}
						size="2.5rem"
					/>
					<span>{info.name}</span>
				</div>
			{/snippet}

			{#snippet right()}
				{#await $me then me}
					{#if me.admin}
						<Link
							data-testid="group-info-edit-link"
							href={`/group-chat/${chatId}/info/edit`}
							iconOnly={theme === 'material'}
						>
							{#if theme === 'material'}
								<wa-icon src={wrapPathInSvg(mdiPencil)}> </wa-icon>
							{:else}
								{m.edit()}
							{/if}
						</Link>
					{/if}
				{/await}
			{/snippet}
		</Navbar>

		{#await $me then me}
			<div class="column" style="flex: 1">
				<div class="column center-in-desktop gap-8 p-2">
					<div class="column" style="align-items: center; gap: 1rem">
						<Avatar
							image={info.image}
							initials={info.name.slice(0, 2)}
							size="5rem"
						>
							<wa-icon src={wrapPathInSvg(mdiAccountGroup)}> </wa-icon>
						</Avatar>

						<span class="text-xl font-semibold">{info.name}</span>

						<span class="quiet">{info.description}</span>
					</div>

					{#await $members then members}
						<BlockTitle>
							{m.membersCount({
								count: Object.keys(members).length,
							})}</BlockTitle
						>
						<ActionList>
							{#if me.admin}
								<ListAction
									title={m.addMembers()}
									icon={mdiPlusCircle}
									href={`/group-chat/${chatId}/info/add-members`}
									data-testid="group-info-add-members"
								/>
							{/if}

							{#each Object.entries(members) as [actorId, member]}
								<!--
								  For member actions, put back:
									onclick={() => (sheetOpenFor = actorId)}
							  -->
								<ListItem link chevron={false} title={member.profile?.name}>
									{#snippet media()}
										<Avatar
											image={member.profile?.avatar}
											initials={member.profile?.name.slice(0, 2)}
											size={32}
										/>
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
										<Avatar
											image={member.profile?.avatar}
											initials={member.profile?.name.slice(0, 2)}
											size={32}
										/>
										<span class="font-semibold">{member.profile?.name}</span>
									</div>

									<ActionList>
										{#if me.admin}
											{#if member.admin}
												<ListAction
													title={m.demoteFromAdministrator()}
													onClick={() => {
														dialogType = 'demote';
														dialogActorId = actorId;
														sheetOpenFor = null;
													}}
													icon={mdiKeyVariant}
												/>
											{:else}
												<ListAction
													title={m.promoteToAdministrator()}
													onClick={() => {
														dialogType = 'promote';
														dialogActorId = actorId;
														sheetOpenFor = null;
													}}
													icon={mdiKeyVariant}
												/>
											{/if}

											<ListAction
												title={m.removeMember()}
												onClick={() => {
													dialogType = 'remove';
													dialogActorId = actorId;
													sheetOpenFor = null;
												}}
												icon={mdiDelete}
											/>
										{/if}
									</ActionList>
								</Sheet>
							{/each}
						</ActionList>

						<ActionList>
							<ListAction
								title={m.leaveGroup()}
								actionType="danger"
								icon={mdiExport}
								onClick={() => (dialogType = 'leave')}
								data-testid="group-info-leave"
							/>

							<!-- <ListAction
								title={m.deleteGroup()}
								actionType="danger"
								icon={mdiClose}
								onClick={() => (dialogType = 'delete')}
							/> -->
						</ActionList>
					{/await}
				</div>
			</div>
		{/await}

		<!-- Dialogs -->
		<!-- <Dialog
			opened={dialogType === 'demote' && dialogActorId !== null}
			onBackdropClick={() => {
				dialogType = null;
				dialogActorId = null;
			}}
			title={m.demoteFromAdministrator()}
		>
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
		</Dialog> -->

		<!-- <Dialog
			opened={dialogType === 'promote' && dialogActorId !== null}
			onBackdropClick={() => {
				dialogType = null;
				dialogActorId = null;
			}}
			title={m.promoteToAdministrator()}
		>
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
		</Dialog> -->

		<!-- <Dialog
			opened={dialogType === 'remove' && dialogActorId !== null}
			onBackdropClick={() => {
				dialogType = null;
				dialogActorId = null;
			}}
			title={m.removeMember()}
		>
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
		</Dialog> -->

		<ActionDialog
			opened={dialogType === 'leave'}
			onCancel={() => (dialogType = null)}
			onConfirm={handleLeaveGroup}
			title={m.leaveGroup()}
			confirmText={m.leave()}
			confirmTestId="group-info-leave-confirm"
		></ActionDialog>

		<!-- <Dialog
			opened={dialogType === 'delete'}
			onBackdropClick={() => (dialogType = null)}
			title={m.deleteGroup()}
		>
			<span>{m.areYouSureDeleteGroup()}</span>
			{#snippet buttons()}
				<DialogButton onClick={() => (dialogType = null)}
					>{m.cancel()}</DialogButton
				>
				<DialogButton strong onClick={handleDeleteGroup} disabled={loading}>
					{loading ? '...' : m.delete()}
				</DialogButton>
			{/snippet}
		</Dialog> -->
	{/await}
</Page>

<style>
	wa-icon {
		width: 32px;
	}
</style>
