<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';

	import {
		mdiAccountMinusOutline,
		mdiAccountMultipleOutline,
		mdiAccountPlusOutline,
		mdiShieldAccountOutline,
	} from '@mdi/js';
	import type { GroupControlEvent } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages';
	import { wrapPathInSvg } from '$lib/utils/icon';

	let { event }: { event: GroupControlEvent } = $props();

	const iconPath = $derived.by(() => {
		switch (event.kind) {
			case 'group_created':
				return mdiAccountMultipleOutline;
			case 'group_member_added':
				return mdiAccountPlusOutline;
			case 'group_member_removed':
				return mdiAccountMinusOutline;
			case 'group_member_promoted':
			case 'group_member_demoted':
				return mdiShieldAccountOutline;
		}
	});
</script>

<div
	class="flex items-center justify-center gap-1.5 py-1 text-sm quiet text-center"
	data-testid={`group-chat-system-message-${event.kind}`}
>
	<wa-icon
		class="shrink-0"
		src={wrapPathInSvg(iconPath)}
		style="font-size: 1rem;"
	></wa-icon>
	<span>
		{#if event.kind === 'group_created'}
			{#if event.isMine}
				{m.youCreatedTheGroup()}
			{:else if event.iAmInitialMember}
				{m.someoneAddedYouToTheGroup({
					name: event.creatorName || m.someone(),
				})}
			{:else if event.creatorName}
				{m.someoneCreatedTheGroup({ name: event.creatorName })}
			{:else}
				{m.groupCreated()}
			{/if}
		{:else if event.kind === 'group_member_added'}
			{#if event.isMine}
				{m.someoneAddedYouToTheGroup({
					name: event.adminName || m.someone(),
				})}
			{:else if event.addedByMe}
				{m.youAddedMember({ name: event.memberName || m.someone() })}
			{:else}
				{m.memberAddedToGroup({
					admin: event.adminName || m.someone(),
					name: event.memberName || m.someone(),
				})}
			{/if}
		{:else if event.kind === 'group_member_removed'}
			{#if event.isMine}
				{m.someoneRemovedYouFromTheGroup({
					name: event.adminName || m.someone(),
				})}
			{:else if event.removedByMe}
				{m.youRemovedMember({ name: event.memberName || m.someone() })}
			{:else if event.memberName || event.adminName}
				{m.memberRemovedFromGroupBy({
					admin: event.adminName || m.someone(),
					name: event.memberName || m.someone(),
				})}
			{:else}
				{m.memberRemovedFromGroup()}
			{/if}
		{:else if event.kind === 'group_member_promoted'}
			{#if event.promotedByMe}
				{m.youMadeMemberAdmin({ name: event.memberName || m.someone() })}
			{:else}
				{m.someoneMadeMemberAdmin({
					admin: event.adminName || m.someone(),
					name: event.memberName || m.someone(),
				})}
			{/if}
		{:else if event.kind === 'group_member_demoted'}
			{#if event.demotedByMe}
				{m.youRevokedAdminFromMember({
					name: event.memberName || m.someone(),
				})}
			{:else}
				{m.someoneRevokedAdminFromMember({
					admin: event.adminName || m.someone(),
					name: event.memberName || m.someone(),
				})}
			{/if}
		{/if}
	</span>
</div>
