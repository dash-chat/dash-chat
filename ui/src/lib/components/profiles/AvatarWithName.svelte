<script lang="ts">
	import { fullName, type Profile } from 'dash-chat-stores';
	import Avatar from '$lib/components/profiles/Avatar.svelte';

	let {
		profile,
		colorSeed,
		nameTestId,
	}: { profile: Profile; colorSeed?: string; nameTestId?: string } = $props();
</script>

<span class="flex w-full min-w-0 flex-row items-center gap-2">
	<span class="shrink-0">
		<Avatar
			image={profile.avatar}
			name={fullName(profile)}
			{colorSeed}
			style="--size: 2.5rem"
		/>
	</span>
	<span class="flex-1 min-w-0 truncate" data-testid={nameTestId}
		>{fullName(profile)}</span
	>
</span>
