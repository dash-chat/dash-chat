import type { Scene } from './scene';

const sceneModules = import.meta.glob<{ default: Scene[] }>(
	'/src/routes/**/scenes.ts',
	{ eager: true },
);

export const scenes: Scene[] = Object.values(sceneModules).flatMap(
	module => module.default,
);
