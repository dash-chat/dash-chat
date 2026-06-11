/** Rendered appearance of the `wa-avatar` inside the given container. */
export function avatarAppearance(
	agent: WebdriverIO.Browser,
	containerSelector: string,
): Promise<{ initials: string; backgroundColor: string; color: string }> {
	return agent.execute((sel: string) => {
		const avatar = document.querySelector(`${sel} wa-avatar`) as
			| (HTMLElement & { initials: string })
			| null;
		if (!avatar) throw new Error(`avatarAppearance: no wa-avatar in ${sel}`);
		const style = getComputedStyle(avatar);
		return {
			initials: avatar.initials,
			backgroundColor: style.backgroundColor,
			color: style.color,
		};
	}, containerSelector);
}
