/** Find the bg div of a Konsta `<Navbar transparent>` whose opacity Konsta
 * mutates to fade in/out on scroll. We override that opacity ourselves
 * (see `ReverseScrollPage.svelte`) and read it in tests, so both sides need
 * to resolve the same element.
 *
 * In iOS theme there are two `.k-navbar > div.absolute` children — a blur
 * layer and the bg. Konsta's `bgElRef` is the LAST one; we match it.
 *
 * TODO: tightly coupled to Konsta v5 internals — re-verify on upgrade.
 */
export function findNavbarBg(root: ParentNode): HTMLElement | null {
	const candidates = root.querySelectorAll('.k-navbar > div.absolute');
	return (candidates[candidates.length - 1] as HTMLElement | undefined) ?? null;
}
