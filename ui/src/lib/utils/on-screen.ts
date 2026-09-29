import type { Attachment } from 'svelte/attachments';

/** Report whether the element is within the viewport, on attach and on every
 * change. Each attached element reports for itself and detaching reports
 * nothing, so a sibling that takes over the same box (an image swapped for
 * its placeholder) keeps the report current. */
export function onScreen(report: (visible: boolean) => void): Attachment {
	return element => {
		const observer = new IntersectionObserver(entries => {
			for (const entry of entries) report(entry.isIntersecting);
		});
		observer.observe(element);
		return () => observer.disconnect();
	};
}
