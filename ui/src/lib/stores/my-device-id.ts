import type { ContactsStore, DeviceId } from 'dash-chat-stores';
import { getContext } from 'svelte';

export function useDeviceId(): DeviceId {
	const contactsStore: ContactsStore = getContext('contacts-store');
	const myDeviceId = contactsStore.myDeviceId().value;
	if (myDeviceId === undefined) {
		throw new Error('useDeviceId called before myDeviceId resolved');
	}
	return myDeviceId;
}
