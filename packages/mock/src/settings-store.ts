import { SettingsStore } from 'dash-chat-stores';
import { type ReactiveFn, type ReactivePromise, reactive } from 'signalium';

import { followingTheMockBackend, mockBackend } from './backend-state';

export class MockSettingsStore extends SettingsStore {
	localHubBlocked: ReactiveFn<ReactivePromise<boolean>, []> = reactive(() =>
		followingTheMockBackend(() => mockBackend.localHubBlocked),
	);
}
