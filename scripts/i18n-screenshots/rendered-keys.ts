/** The key the app renders a source string under: i18next keeps each plural
 *  form of a message under its own suffixed key. */
export function renderedKey(sourceKey: string): string {
	return sourceKey.replace(/_(zero|one|two|few|many|other)$/, '');
}

/** Every key the app can render, from its source strings by key. */
export function renderedSourceKeys(
	sourceStrings: Record<string, string>,
): string[] {
	return [...new Set(Object.keys(sourceStrings).map(renderedKey))];
}
