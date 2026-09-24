import { modalHost } from '$lib/stores/modal-host.svelte';
import type { Attachment } from 'svelte/attachments';

// Overlays are `position: fixed`. The virtual keyboard's FLIP writes inline
// transforms on the message list and the composer bar, which makes any fixed
// descendant resolve against them instead of the viewport.
export const portalToModalHost: Attachment<HTMLElement> = node => {
	const host = modalHost.element;
	if (!host) return;
	host.append(node);
	return () => node.remove();
};
