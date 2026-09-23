import { modalHost } from '$lib/stores/modal-host.svelte';

/** Move `node` into the modal host while it is mounted.
 *
 * Konsta overlays are `position: fixed`. The virtual keyboard's FLIP writes
 * inline transforms on the message list and the composer bar, and the
 * spotlight lifts a message with one, which makes any fixed descendant resolve
 * against them instead of the viewport. */
export function portalToModalHost(node: HTMLElement) {
	const host = modalHost.element;
	if (!host) return;
	host.append(node);
	return () => node.remove();
}
