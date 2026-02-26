import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { type UnsubscribeFunction } from 'emittery';

export interface Settings {
	qr_color: string | null;
	color_scheme: string | null;
	local_mailbox_enabled: boolean;
}

export interface ISettingsClient {
	getSettings(): Promise<Settings>;
	setSetting(key: string, value: unknown): Promise<void>;
	onSettingsUpdated(handler: (settings: Settings) => void): UnsubscribeFunction;
}

export class SettingsClient implements ISettingsClient {
	getSettings(): Promise<Settings> {
		return invoke('get_settings');
	}

	setSetting(key: string, value: unknown): Promise<void> {
		return invoke('set_setting', { key, value });
	}

	onSettingsUpdated(handler: (settings: Settings) => void): UnsubscribeFunction {
		let unsubs: (() => void) | undefined;
		listen('settings://updated', (e) => {
			handler(e.payload as Settings);
		}).then((u) => (unsubs = u));

		return () => {
			if (unsubs) unsubs();
		};
	}
}
