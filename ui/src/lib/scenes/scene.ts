import type { Scene as SceneOfAnyDevice } from 'translation-screenshots/scene';

/** One state of the UI worth showing a translator: its `setup` brings the
 * browser preview there, writing to the mock backend what it needs on top of
 * the demo data and doing to the page what a user would. An Android phone
 * unless it names another device. */
export type Scene = SceneOfAnyDevice<'iphone' | 'desktop' | 'linux-desktop'>;
