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
	import RemoveMemberDialog from '$lib/components/group-chats/RemoveMemberDialog.svelte';
	import LeaveGroupDialog from '$lib/components/group-chats/LeaveGroupDialog.svelte';
	let chatId = page.params.chatId!;

	const chatsStore: ChatsStore = getContext('chats-store');
	const groupChatStore = chatsStore.groupChats(chatId);

	const info = useReactivePromise(groupChatStore.info);
	const members = useReactivePromise(groupChatStore.allMembers);
	const me = useReactivePromise(groupChatStore.me);

	let sheetOpenFor = $state<string | null>(null);
	let removeMemberDialog = $state<RemoveMemberDialog>();
	let leaveGroupDialog = $state<LeaveGroupDialog>();
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

						<span
							class="text-xl font-semibold break-words text-center max-w-full"
							>{info.name}</span
						>

						<span class="quiet break-words text-center max-w-full"
							>{info.description}</span
						>
					</div>

					{#await $members then members}
						<BlockTitle>
							{m.membersCount({
								count: Object.keys(members).length,
							})}</BlockTitle
						>
						<ActionList data-testid="group-info-members">
							{#if me.admin}
								<ListAction
									title={m.addMembers()}
									icon={mdiPlusCircle}
									href={`/group-chat/${chatId}/info/add-members`}
									data-testid="group-info-add-members"
								/>
							{/if}

							{#each Object.entries(members) as [actorId, member]}
								<ListItem
									link
									chevron={false}
									title={member.profile?.name}
									data-testid={`group-info-member-${member.profile?.name}`}
									onclick={me.admin
										? () => (sheetOpenFor = actorId)
										: undefined}
								>
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
							{/each}
						</ActionList>

						{#if me.member}
							<ActionList>
								<ListAction
									title={m.leaveGroup()}
									actionType="danger"
									icon={mdiExport}
									onClick={() => leaveGroupDialog?.show()}
									data-testid="group-info-leave"
								/>

								<!-- <ListAction
								title={m.deleteGroup()}
								actionType="danger"
								icon={mdiClose}
								onClick={() => (dialogType = 'delete')}
							/> -->
							</ActionList>
						{/if}

						{@const sheetMember = sheetOpenFor ? members[sheetOpenFor] : null}
						<Sheet
							class="pb-safe"
							opened={sheetOpenFor !== null}
							onBackdropClick={() => (sheetOpenFor = null)}
						>
							{#if sheetMember}
								<div
									class="flex-col gap-4 py-4"
									style="display: flex; align-items: center;"
								>
									<Avatar
										image={sheetMember.profile?.avatar}
										initials={sheetMember.profile?.name.slice(0, 2)}
										size={32}
									/>
									<span class="font-semibold">{sheetMember.profile?.name}</span>
								</div>

								<ActionList>
									{#if me.admin}
										<!-- {#if sheetMember.admin}
											<ListAction
												title={m.demoteFromAdministrator()}
												onClick={() => {
													dialogType = 'demote';
													dialogActorId = sheetOpenFor;
													sheetOpenFor = null;
												}}
												icon={mdiKeyVariant}
											/>
										{:else}
											<ListAction
												title={m.promoteToAdministrator()}
												onClick={() => {
													dialogType = 'promote';
													dialogActorId = sheetOpenFor;
													sheetOpenFor = null;
												}}
												icon={mdiKeyVariant}
											/>
										{/if} -->

										{#if sheetOpenFor === me.agentId}
											<ListAction
												title={m.leaveGroup()}
												actionType="danger"
												icon={mdiExport}
												onClick={() => leaveGroupDialog?.show()}
												data-testid="group-info-leave-self"
											/>
										{:else}
											<ListAction
												title={m.removeMember()}
												onClick={() => {
													if (sheetOpenFor) {
														removeMemberDialog?.show(sheetOpenFor);
													}
													sheetOpenFor = null;
												}}
												icon={mdiDelete}
												data-testid="group-info-remove-member"
											/>
										{/if}
									{/if}
								</ActionList>
							{/if}
						</Sheet>
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

		<RemoveMemberDialog bind:this={removeMemberDialog} {chatId} />

		<LeaveGroupDialog bind:this={leaveGroupDialog} {chatId} />

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
