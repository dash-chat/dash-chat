<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { fullName, type Profile } from 'dash-chat-stores';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCancel } from '@mdi/js';

	let {
		profile,
		nameTestId,
		blocked = false,
	}: { profile: Profile; nameTestId?: string; blocked?: boolean } = $props();
</script>

<span class="flex w-full min-w-0 flex-row items-center gap-2">
	<span class="shrink-0">
		<Avatar
			image={profile.avatar}
			initials={profile.name.slice(0, 2)}
			size="2.5rem"
		/>
	</span>
	<span
		class="flex min-w-0 flex-1 flex-row items-center gap-1"
		data-testid={nameTestId}
	>
		{#if blocked}
			<wa-icon
				class="small-icon quiet shrink-0"
				src={wrapPathInSvg(mdiCancel)}
				data-testid="blocked-name-icon"
			></wa-icon>
		{/if}
		<span class="truncate min-w-0">{fullName(profile)}</span>
	</span>
</span>
