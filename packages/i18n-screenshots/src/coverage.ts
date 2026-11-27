export interface StringsNotAccountedFor {
	/** Source strings no scene shows that are not listed as such. */
	unlisted: string[];
	/** Strings listed as shown by no scene that one does show, or that are
	 * no longer source strings. */
	stale: string[];
}

/** Which source strings are neither shown by a scene nor listed as ones no
 * scene can show, and which are listed for nothing: the list is how a new
 * string without a screenshot gets noticed, so it may only shrink. */
export function stringsNotAccountedFor(
	shownKeys: Set<string>,
	sourceKeys: string[],
	keysListedWithoutScreenshot: string[],
): StringsNotAccountedFor {
	const notShown = sourceKeys.filter(key => !shownKeys.has(key));
	return {
		unlisted: notShown.filter(
			key => !keysListedWithoutScreenshot.includes(key),
		),
		stale: keysListedWithoutScreenshot.filter(key => !notShown.includes(key)),
	};
}
