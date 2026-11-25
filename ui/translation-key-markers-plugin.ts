import path from 'node:path';
import type { Plugin } from 'vite';

const MARKERS_MODULE = path.resolve(
	import.meta.dirname,
	'tests/translation-key-markers.ts',
);

/** Routes every compiled paraglide message through `markTranslationKey`, so the
 * translation screenshots can read which message keys are on screen. */
export function translationKeyMarkers(): Plugin {
	return {
		name: 'translation-key-markers',
		transform(code, id) {
			if (!/\/paraglide\/messages\/[^/]+\.js$/.test(id.split('?')[0])) return;

			// Paraglide compiles a key that isn't a valid identifier under a
			// mangled name and re-exports it under the key.
			const keysByCompiledName = new Map(
				[...code.matchAll(/export \{ (\w+) as "([^"]+)" \}/g)].map(
					([, compiledName, key]) => [compiledName, key],
				),
			);
			const marked = code.replace(
				/trackMessageCall\("(\w+)", locale\)([\s\S]*?)\n\};/g,
				(message: string, compiledName: string, localeReturns: string) => {
					const key = keysByCompiledName.get(compiledName) ?? compiledName;
					return message.replace(
						localeReturns,
						localeReturns.replace(
							/return ([\w.]+\(inputs\))/g,
							`return markTranslationKey(${JSON.stringify(key)}, $1)`,
						),
					);
				},
			);
			if (marked === code) return;
			return `import { markTranslationKey } from ${JSON.stringify(MARKERS_MODULE)};\n${marked}`;
		},
	};
}
