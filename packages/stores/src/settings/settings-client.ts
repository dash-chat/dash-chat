import { invoke } from '@tauri-apps/api/core';

export interface Settings {
	qr_color: string | null;
	color_scheme: string | null;
	local_mailbox_enabled: boolean;
}

export interface ISettingsClient {
	getSettings(): Promise<Settings>;
	setSetting(key: string, value: unknown): Promise<void>;
}

export class SettingsClient implements ISettingsClient {
	getSettings(): Promise<Settings> {
		return invoke('get_settings');
	}

	setSetting(key: string, value: unknown): Promise<void> {
		return invoke('set_setting', { key, value });
	}
}
