<script lang="ts">
	import { mdiAccountOutline } from '@mdi/js';
	import type { Profile } from 'dash-chat-stores';
	import Avatar from './Avatar.svelte';

	let {
		profile,
		size,
		style,
		testId,
	}: {
		/** Unset while the peer's profile has not arrived. */
		profile?: Pick<Profile, 'name' | 'avatar'> | undefined;
		size?: number | string | undefined;
		style?: string | undefined;
		testId?: string | undefined;
	} = $props();
</script>

<Avatar
	image={profile?.avatar}
	initials={profile?.name.slice(0, 2)}
	fallbackIcon={profile ? undefined : mdiAccountOutline}
	{size}
	{style}
	{testId}
/>
