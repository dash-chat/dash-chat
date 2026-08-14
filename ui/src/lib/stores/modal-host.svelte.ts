let host = $state<HTMLElement | null>(null);

/**
 * The element every `Modal` portals into. It lives inside Konsta's `<App>`
 * because `--k-safe-area-*` is defined on that element's `.safe-areas` class —
 * a host on `document.body` would silently zero every `pb-safe` / `left-safe`.
 */
export const modalHost = {
	get element() {
		return host;
	},
	set element(el: HTMLElement | null) {
		host = el;
	},
};
