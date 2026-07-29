/**
 * Keep an overlay out of the DOM until it is needed, without losing the
 * animations it plays on the way in and out. Render the overlay under
 * `mounted` and hand it the returned `opened`: that turns true a couple of
 * frames after mounting, and back to false `exitDuration` before unmounting.
 */
export function lazyMount(opened: () => boolean, exitDuration = 400) {
	let mounted = $state(false);
	let open = $state(false);

	$effect(() => {
		if (!opened()) {
			open = false;
			const timeout = setTimeout(() => (mounted = false), exitDuration);
			return () => clearTimeout(timeout);
		}

		mounted = true;
		// Opening on the very next frame would land in the same style recalc as
		// the mount, leaving the browser nothing to transition from.
		let second = 0;
		const first = requestAnimationFrame(() => {
			second = requestAnimationFrame(() => (open = true));
		});
		return () => {
			cancelAnimationFrame(first);
			cancelAnimationFrame(second);
		};
	});

	return {
		get mounted() {
			return mounted;
		},
		get opened() {
			return open;
		},
	};
}
