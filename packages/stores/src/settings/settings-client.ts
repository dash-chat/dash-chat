import { listen } from '@tauri-apps/api/event';
import { type UnsubscribeFunction } from 'emittery';

import { invoke } from '../utils/invoke';

export interface Settings {
	qr_color: string | null;
	color_scheme: string | null;
	local_mailbox_enabled: boolean;
	notifications_enabled: boolean;
}

export interface ISettingsClient {
	getSettings(): Promise<Settings>;
	setSetting(key: string, value: unknown): Promise<void>;
	setLocalMailboxEnabled(enabled: boolean): Promise<void>;
	setNotificationsEnabled(enabled: boolean): Promise<void>;
	onSettingsUpdated(handler: (settings: Settings) => void): UnsubscribeFunction;
}

export class SettingsClient implements ISettingsClient {
	getSettings(): Promise<Settings> {
		return invoke('get_settings');
	}

	setSetting(key: string, value: unknown): Promise<void> {
		return invoke('set_setting', { key, value });
	}

	setLocalMailboxEnabled(enabled: boolean): Promise<void> {
		return invoke('set_local_mailbox_enabled', { enabled });
	}

	setNotificationsEnabled(enabled: boolean): Promise<void> {
		return this.setSetting('notifications_enabled', enabled);
	}

	onSettingsUpdated(
		handler: (settings: Settings) => void,
	): UnsubscribeFunction {
		let unsubs: (() => void) | undefined;
		let cancelled = false;
		listen('settings://updated', e => {
			handler(e.payload as Settings);
		}).then(u => {
			if (cancelled) u();
			else unsubs = u;
		});

		return () => {
			cancelled = true;
			if (unsubs) unsubs();
		};
	}
}
