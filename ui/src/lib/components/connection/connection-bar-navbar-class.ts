/** Slides a page's `Navbar` out from under the open `ConnectionStatusBar`. A
 * transform, because growing the navbar would change the height the chat scroll
 * container compensates for, moving the messages under the reader. */
export function connectionBarNavbarClass(barOpened: boolean): string {
	return `transition-transform duration-200 ${barOpened ? 'translate-y-14' : ''}`;
}
