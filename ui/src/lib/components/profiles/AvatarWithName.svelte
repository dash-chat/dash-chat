<script lang="ts">
	import { fullName, type Profile } from 'dash-chat-stores';
	import Avatar from '$lib/components/profiles/Avatar.svelte';

	let { profile, nameTestId }: { profile: Profile; nameTestId?: string } =
		$props();
</script>

<span class="flex w-full min-w-0 flex-row items-center gap-2">
	<span class="shrink-0">
		<Avatar
			image={profile.avatar}
			initials={profile.name.slice(0, 2)}
			size="2.5rem"
		/>
	</span>
	<span class="flex-1 min-w-0 truncate" data-testid={nameTestId}
		>{fullName(profile)}</span
	>
</span>
