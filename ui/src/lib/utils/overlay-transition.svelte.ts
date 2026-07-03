/** Drives the presence of a Konsta overlay (Sheet, Dialog, Popup, …) that
 * should live in the DOM only around being open. Konsta animates overlays by
 * swapping classes on an always-mounted element and has no mount/unmount
 * transition support, so lazily mounted overlays need staging: `mounted`
 * gates the DOM (`{#if transition.mounted}`) and `shown` must drive the
 * Konsta `opened` prop. `shown` turns on two frames after mount (the closed
 * state must be painted first or the enter transition is skipped) and off
 * `duration`ms before unmount so the exit transition can run.
 *
 * Must be constructed during component init (it owns an `$effect`).
 */
export class OverlayTransition {
	mounted = $state(false);
	shown = $state(false);

	constructor(opened: () => boolean, duration = 400) {
		$effect(() => {
			if (opened()) {
				this.mounted = true;
				let raf2 = 0;
				const raf1 = requestAnimationFrame(() => {
					raf2 = requestAnimationFrame(() => (this.shown = true));
				});
				return () => {
					cancelAnimationFrame(raf1);
					cancelAnimationFrame(raf2);
				};
			}
			this.shown = false;
			const timeout = setTimeout(() => (this.mounted = false), duration);
			return () => clearTimeout(timeout);
		});
	}
}
