import {
	type ReactiveFn,
	type ReactivePromise,
	reactive,
	relay,
} from 'signalium';

import type {
	ColorScheme,
	ColorSchemePreference,
	ISettingsClient,
	Settings,
} from './settings-client.js';

export class SettingsStore {
	constructor(public client: ISettingsClient) {}

	private settings = reactive(() =>
		relay<Settings>(state => {
			this.client.getSettings().then(s => {
				state.value = s;
			});

			const unsubs = this.client.onSettingsUpdated(settings => {
				state.value = settings;
			});

			return unsubs;
		}),
	);

	colorSchemePreference: ReactiveFn<
		ReactivePromise<ColorSchemePreference>,
		[]
	> = reactive(() => this.client.colorSchemePreference());

	colorScheme = reactive((): ColorScheme => this.client.colorScheme());

	qrColor = reactive(async (): Promise<string | null> => {
		const settings = await this.settings();
		return settings.qr_color || null;
	});

	localMailboxEnabled = reactive(async () => {
		const settings = await this.settings();
		return settings.local_mailbox_enabled;
	});

	notificationsEnabled = reactive(async () => {
		const settings = await this.settings();
		return settings.notifications_enabled;
	});

	backgroundModeEnabled = reactive(async () => {
		const settings = await this.settings();
		return settings.background_mode_enabled;
	});

	async setColorSchemePreference(scheme: ColorSchemePreference): Promise<void> {
		await this.client.setColorSchemePreference(scheme);
	}

	async setQrColor(color: string): Promise<void> {
		await this.client.setSetting('qr_color', color);
	}

	async setBackgroundModeEnabled(enabled: boolean): Promise<void> {
		await this.client.setBackgroundModeEnabled(enabled);
	}

	async setLocalMailboxEnabled(enabled: boolean): Promise<void> {
		await this.client.setLocalMailboxEnabled(enabled);
	}

	async setNotificationsEnabled(enabled: boolean): Promise<void> {
		await this.client.setNotificationsEnabled(enabled);
	}
}
