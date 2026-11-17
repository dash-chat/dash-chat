<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { fullName, type Profile } from 'dash-chat-stores';
	import type { Snippet } from 'svelte';
	import PeerAvatar from '$lib/components/profiles/PeerAvatar.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCancel } from '@mdi/js';

	let {
		profile,
		nameTestId,
		blocked = false,
		subtitle,
	}: {
		profile: Profile;
		nameTestId?: string;
		blocked?: boolean;
		subtitle?: Snippet;
	} = $props();
</script>

<span class="flex w-full min-w-0 flex-row items-center gap-4">
	<span class="shrink-0">
		<PeerAvatar {profile} size="2.25rem" />
	</span>
	<span class="flex min-w-0 flex-1 flex-col">
		<span
			class="flex min-w-0 flex-row items-center gap-1"
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
		{@render subtitle?.()}
	</span>
</span>
