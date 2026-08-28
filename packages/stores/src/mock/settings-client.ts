import Emittery from 'emittery';
import { type ReactivePromise, reactive, signal } from 'signalium';
import { colorScheme as appliedColorScheme } from 'tauri-plugin-system-theme';

import type {
	ColorScheme,
	ColorSchemePreference,
	ISettingsClient,
	Settings,
} from '../settings/settings-client';

export class MockSettingsClient implements ISettingsClient {
	private colorSchemePreferenceSignal = signal<ColorSchemePreference>('system');

	private colorSchemePreferenceReactive = reactive(
		async (): Promise<ColorSchemePreference> =>
			this.colorSchemePreferenceSignal.value,
	);

	colorSchemePreference(): ReactivePromise<ColorSchemePreference> {
		return this.colorSchemePreferenceReactive();
	}

	colorScheme(): ColorScheme {
		const scheme = this.colorSchemePreferenceSignal.value;
		// Resolving `system` is the plugin's rule, and it needs no Tauri runtime.
		return scheme === 'system' ? appliedColorScheme() : scheme;
	}

	async setColorSchemePreference(scheme: ColorSchemePreference): Promise<void> {
		this.colorSchemePreferenceSignal.value = scheme;
	}

	private settings: Settings = {
		qr_color: null,
		local_mailbox_enabled: false,
		notifications_enabled: false,
		developer_mode_enabled: false,
	};

	private emitter = new Emittery<{ updated: Settings }>();

	async getSettings(): Promise<Settings> {
		return { ...this.settings };
	}

	async setSetting(key: string, value: unknown): Promise<void> {
		(this.settings as unknown as Record<string, unknown>)[key] = value;
		this.emitter.emit('updated', { ...this.settings });
	}

	async setLocalMailboxEnabled(enabled: boolean): Promise<void> {
		this.settings.local_mailbox_enabled = enabled;
		this.emitter.emit('updated', { ...this.settings });
	}

	async setNotificationsEnabled(enabled: boolean): Promise<void> {
		this.settings.notifications_enabled = enabled;
		this.emitter.emit('updated', { ...this.settings });
	}

	onSettingsUpdated(handler: (settings: Settings) => void): () => void {
		return this.emitter.on('updated', handler);
	}
}
