import type { SceneToCapture } from '../../../tests/setup-utils';
import type { Scene } from './scene';

const sceneModules = import.meta.glob<{ default: Scene[] }>(
	'/src/routes/**/scenes.ts',
	{ eager: true },
);

export const scenes: Scene[] = Object.values(sceneModules).flatMap(
	module => module.default,
);

/** The scenes as the screenshot runner gets them: a function cannot leave the
 * page, so a step that is one is named by its position. */
export function scenesToCapture(): SceneToCapture[] {
	return scenes.map(({ name, path, device, steps = [], drawn = [] }) => ({
		name,
		path,
		device,
		drawn,
		steps: steps.map((step, index) =>
			typeof step === 'function' ? { run: index } : step,
		),
	}));
}

export async function runRequestedSceneStep(index: number): Promise<void> {
	const step = requestedScene()?.steps?.[index];
	if (typeof step === 'function') await step();
}

/** The scene the `?scene=` query parameter asks for. */
export function requestedScene(): Scene | undefined {
	const name = new URLSearchParams(location.search).get('scene');
	return scenes.find(scene => scene.name === name);
}
