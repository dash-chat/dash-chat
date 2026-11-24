import type { AgentId, ContactsStore } from 'dash-chat-stores';
import { getContext } from 'svelte';

export function useMyAgentId(): AgentId {
	const contactsStore: ContactsStore = getContext('contacts-store');
	const myAgentId = contactsStore.myAgentId().value;
	if (myAgentId === undefined) {
		throw new Error('useMyAgentId called before myAgentId resolved');
	}
	return myAgentId;
}
