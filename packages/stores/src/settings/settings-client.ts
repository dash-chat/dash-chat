import { listen } from '@tauri-apps/api/event';
import { type UnsubscribeFunction } from 'emittery';
import { type ReactivePromise } from 'signalium';
import {
	type ColorScheme,
	type ColorSchemePreference,
	colorScheme,
	colorSchemePreference,
	setColorSchemePreference,
} from 'tauri-plugin-system-theme';

import { invokeAfterSetup } from '../utils/invoke-after-setup';

export interface Settings {
	qr_color: string | null;
	local_mailbox_enabled: boolean;
	notifications_enabled: boolean;
	background_mode_enabled: boolean;
}

export type { ColorScheme, ColorSchemePreference };

export interface ISettingsClient {
	colorSchemePreference(): ReactivePromise<ColorSchemePreference>;
	colorScheme(): ColorScheme;
	setColorSchemePreference(scheme: ColorSchemePreference): Promise<void>;

	getSettings(): Promise<Settings>;
	setSetting(key: string, value: unknown): Promise<void>;
	setLocalMailboxEnabled(enabled: boolean): Promise<void>;
	setNotificationsEnabled(enabled: boolean): Promise<void>;
	setBackgroundModeEnabled(enabled: boolean): Promise<void>;
	onSettingsUpdated(handler: (settings: Settings) => void): UnsubscribeFunction;
}

export class SettingsClient implements ISettingsClient {
	colorSchemePreference(): ReactivePromise<ColorSchemePreference> {
		return colorSchemePreference();
	}

	colorScheme(): ColorScheme {
		return colorScheme();
	}

	setColorSchemePreference(scheme: ColorSchemePreference): Promise<void> {
		return setColorSchemePreference(scheme);
	}

	getSettings(): Promise<Settings> {
		return invokeAfterSetup('get_settings');
	}

	setSetting(key: string, value: unknown): Promise<void> {
		return invokeAfterSetup('set_setting', { key, value });
	}

	setLocalMailboxEnabled(enabled: boolean): Promise<void> {
		return invokeAfterSetup('set_local_mailbox_enabled', { enabled });
	}

	setNotificationsEnabled(enabled: boolean): Promise<void> {
		return this.setSetting('notifications_enabled', enabled);
	}

	setBackgroundModeEnabled(enabled: boolean): Promise<void> {
		return this.setSetting('background_mode_enabled', enabled);
	}

	onSettingsUpdated(
		handler: (settings: Settings) => void,
	): UnsubscribeFunction {
		return listenSync('settings://updated', handler);
	}
}

function listenSync<T>(
	event: string,
	handler: (payload: T) => void,
): UnsubscribeFunction {
	const unlisten = listen<T>(event, e => {
		handler(e.payload);
	}).catch(e => {
		console.error(`Failed to listen to ${event}`, e);
		return () => {};
	});

	return () => {
		unlisten.then(u => u());
	};
}
