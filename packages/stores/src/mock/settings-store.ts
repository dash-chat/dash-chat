import { type ReactiveFn, type ReactivePromise, reactive } from 'signalium';

import { SettingsStore } from '../settings/settings-store';
import { mockBackend } from './backend-state';

export class MockSettingsStore extends SettingsStore {
	localHubBlocked: ReactiveFn<ReactivePromise<boolean>, []> = reactive(
		async () => mockBackend.localHubBlocked,
	);
}
