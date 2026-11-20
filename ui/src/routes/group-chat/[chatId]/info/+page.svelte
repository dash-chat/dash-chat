<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type {
		ChatsStore,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { fullName } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiExport, mdiKeyVariant, mdiPencil, mdiPlusCircle } from '@mdi/js';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		ListItem,
		Chip,
		Dialog,
		BlockTitle,
		useTheme,
		Link,
	} from 'konsta/svelte';
	import SwipeableSheet from '$lib/components/SwipeableSheet.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';

	import { page } from '$app/state';
	import GroupAvatar from '$lib/components/profiles/GroupAvatar.svelte';
	import PeerAvatar from '$lib/components/profiles/PeerAvatar.svelte';
	import P2pBadge from '$lib/components/connection/P2pBadge.svelte';
	import { peersConnectedOverP2p } from '$lib/components/connection/connection-status';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import ListAction from '$lib/components/navigation/ListAction.svelte';
	import ActionList from '$lib/components/navigation/ActionList.svelte';
	import MakeAdminDialog from '$lib/components/chats/MakeAdminDialog.svelte';
	import RemoveMemberDialog from '$lib/components/chats/RemoveMemberDialog.svelte';
	import LeaveGroupDialog from '$lib/components/chats/LeaveGroupDialog.svelte';
	let chatId = page.params.chatId!;

	const chatsStore: ChatsStore = getContext('chats-store');
	const groupChatStore = chatsStore.groupChats(chatId);

	const info = useReactivePromise(groupChatStore.info);
	const members = useReactivePromise(groupChatStore.allMembers);
	const me = useReactivePromise(groupChatStore.me);

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const onlinePeersStore: IOnlinePeersStore = getContext('online-peers-store');
	const mailboxes = useReactiveValue(
		mailboxTrackerStore.connectionStatusForAllMailboxes,
	);
	const onlinePeers = useReactiveValue(onlinePeersStore.onlinePeers, chatId);
	const membersActiveOverP2p = $derived(
		$mailboxes !== undefined && $onlinePeers !== undefined
			? peersConnectedOverP2p(
					$mailboxes,
					$onlinePeers,
					renderingResumedAt.value,
				)
			: [],
	);

	// selectedMemberId outlives the close so the member's actions stay
	// rendered while the sheet or dialog animates out.
	let selectedMemberId = $state<string | null>(null);
	let memberActionsOpened = $state(false);

	function openMemberActions(actorId: string) {
		selectedMemberId = actorId;
		memberActionsOpened = true;
	}
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

<Page data-testid="group-info-page">
	{#await $info then info}
		<Navbar
			transparent={true}
			titleClass="min-w-0 flex-1"
			leftClass="shrink-0"
			rightClass="shrink-0"
			centerTitle={false}
		>
			{#snippet left()}
				<NavbarBackLink
					data-testid="group-info-back"
					onClick={() => goto(`/group-chat/${chatId}`)}
				/>
			{/snippet}

			{#snippet title()}
				<div class="flex w-full min-w-0 items-center justify-start gap-2">
					<span class="shrink-0">
						<GroupAvatar image={info.image} size="2.5rem" />
					</span>
					<span
						class="flex-1 min-w-0 truncate"
						data-testid="group-info-header-name">{info.name}</span
					>
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
						<GroupAvatar
							image={info.image}
							size="5rem"
							testId="group-info-avatar"
						/>

						<span
							class="text-xl font-semibold break-words text-center max-w-full"
							data-testid="group-info-name">{info.name}</span
						>

						<span
							class="quiet break-words text-center max-w-full"
							data-testid="group-info-description">{info.description}</span
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
								{@const activeOverP2p = membersActiveOverP2p.includes(actorId)}
								<ListItem
									link={me.admin && actorId !== me.agentId}
									chevron={false}
									strongTitle={false}
									innerClass="py-0! self-stretch flex flex-col justify-center"
									data-testid={`group-info-member-${member.profile?.name}`}
									onclick={me.admin && actorId !== me.agentId
										? () => openMemberActions(actorId)
										: undefined}
								>
									{#snippet media()}
										<P2pBadge
											show={activeOverP2p}
											testId={`group-info-member-p2p-badge-${member.profile?.name}`}
										>
											<PeerAvatar profile={member.profile} size={32} />
										</P2pBadge>
									{/snippet}

									<!-- Konsta centers `after` on the title line only, which
									     leaves the chip off-center once a subtitle shows. -->
									{#snippet inner()}
										<div
											class="row items-center justify-between gap-1 in-[.k-ios]:min-h-7"
										>
											<div
												class="column min-w-0 text-[16px] in-[.k-ios]:text-[17px]"
											>
												<span class="leading-5">
													{actorId === me.agentId
														? m.you()
														: member.profile?.name}
												</span>
												{#if activeOverP2p}
													<span class="text-xs">{m.activeOverP2p()}</span>
												{/if}
											</div>
											{#if member.admin}
												<Chip
													class="shrink-0"
													data-testid="group-info-member-admin-chip"
													>{m.administrator()}</Chip
												>
											{/if}
										</div>
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
						{/if}

						{@const selectedMember = selectedMemberId
							? members[selectedMemberId]
							: null}
						{@const memberActionsVisible =
							memberActionsOpened && !!selectedMember && me.admin}
						{#snippet memberActions(listClass?: string)}
							{#if selectedMember}
								<ActionList class={listClass}>
									{#if me.admin}
										<!-- {#if selectedMember.admin}
											<ListAction
												title={m.demoteFromAdministrator()}
												onClick={() => {
													dialogType = 'demote';
													dialogActorId = selectedMemberId;
													memberActionsOpened = false;
												}}
												icon={mdiKeyVariant}
											/>
										{/if} -->
										{#if !selectedMember.admin}
											<ListAction
												title={m.makeAdmin()}
												onClick={() => {
													dialogType = 'promote';
													dialogActorId = selectedMemberId;
													memberActionsOpened = false;
												}}
												icon={mdiKeyVariant}
												data-testid="group-info-make-admin"
											/>
										{/if}

										<ListAction
											title={m.removeMember()}
											onClick={() => {
												dialogType = 'remove';
												dialogActorId = selectedMemberId;
												memberActionsOpened = false;
											}}
											icon={mdiExport}
											data-testid="group-info-remove-member"
										/>
									{/if}
								</ActionList>
							{/if}
						{/snippet}

						{#if isWideScreen.value}
							<Dialog
								opened={memberActionsVisible}
								onBackdropClick={() => (memberActionsOpened = false)}
							>
								{#if selectedMember}
									<div class="flex flex-col items-center gap-3 px-4 pt-2 pb-5">
										<PeerAvatar profile={selectedMember.profile} size={80} />
										<span
											class="text-xl font-medium break-words text-center max-w-full"
											>{selectedMember.profile
												? fullName(selectedMember.profile)
												: m.someone()}</span
										>
									</div>
								{/if}
								{@render memberActions('!mx-0')}
							</Dialog>
						{:else}
							<SwipeableSheet
								class="pb-safe"
								opened={memberActionsVisible}
								onClose={() => (memberActionsOpened = false)}
							>
								{#if selectedMember}
									<div class="flex flex-col items-center gap-3 px-4 pt-6 pb-4">
										<PeerAvatar profile={selectedMember.profile} size={80} />
										<span
											class="text-xl font-medium break-words text-center max-w-full"
											>{selectedMember.profile
												? fullName(selectedMember.profile)
												: m.someone()}</span
										>
									</div>
								{/if}
								{@render memberActions()}
							</SwipeableSheet>
						{/if}

						{@const dialogMemberProfile =
							dialogActorId !== null
								? members[dialogActorId]?.profile
								: undefined}
						<!-- dialogActorId outlives the close so the member's name stays in
						     the description while the dialog animates out. -->
						<MakeAdminDialog
							bind:opened={
								() => dialogType === 'promote' && dialogActorId !== null,
								opened => {
									if (!opened) dialogType = null;
								}
							}
							{chatId}
							actorId={dialogActorId}
							memberName={dialogMemberProfile
								? fullName(dialogMemberProfile)
								: undefined}
						/>
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

		<RemoveMemberDialog
			bind:opened={
				() => dialogType === 'remove' && dialogActorId !== null,
				opened => {
					if (!opened) {
						dialogType = null;
						dialogActorId = null;
					}
				}
			}
			{chatId}
			actorId={dialogActorId}
		/>

		<LeaveGroupDialog
			bind:opened={
				() => dialogType === 'leave',
				opened => {
					if (!opened) dialogType = null;
				}
			}
			{chatId}
		/>

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
