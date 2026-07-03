<script lang="ts">
	import { ListItem } from 'konsta/svelte';
	import { reactive } from 'signalium';
	import {
		fullName,
		type ContactsStore,
		type DeviceId,
		type MessagesStore,
	} from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import Avatar from '$lib/components/profiles/Avatar.svelte';

	let {
		deviceId,
		emoji,
		own,
		removable,
		onRemove,
	}: {
		deviceId: DeviceId;
		emoji: string;
		own: boolean;
		removable: boolean;
		onRemove: () => void;
	} = $props();

	const store: MessagesStore = getContext('messages-store');
	const contactsStore: ContactsStore = getContext('contacts-store');

	const deviceProfile = reactive(async (id: DeviceId) => {
		const agentId = await store.agentIdForDeviceId(id);
		if (agentId === undefined) return undefined;
		return await contactsStore.profiles(agentId);
	});
	const profile = $derived(useReactiveValue(deviceProfile, deviceId));
</script>

<ListItem
	link={removable}
	chevron={false}
	title={own ? m.you() : $profile ? fullName($profile) : m.unknownSender()}
	subtitle={removable ? m.tapToRemove() : undefined}
	onClick={removable ? onRemove : undefined}
	data-testid={own ? 'reaction-row-own' : 'reaction-row'}
>
	{#snippet media()}
		{#if $profile}
			<Avatar
				image={$profile.avatar}
				initials={$profile.name.slice(0, 2)}
				size="2.5rem"
			/>
		{:else}
			<Avatar waitingForProfile size="2.5rem" />
		{/if}
	{/snippet}
	{#snippet after()}
		<span class="text-xl">{emoji}</span>
	{/snippet}
</ListItem>
