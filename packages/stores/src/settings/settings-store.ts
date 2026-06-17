import { reactive, relay, signal } from 'signalium';

import type { ISettingsClient, Settings } from './settings-client.js';

export type ColorScheme = 'light' | 'dark' | 'system';

/** Default QR color — mirrors the brand primary (`--color-brand-primary`). */
export const DEFAULT_QR_COLOR = '#6e7bff';

export class SettingsStore {
	private systemDarkSignal = signal<boolean>(false);

	constructor(public client: ISettingsClient) {
		this.listenSystemTheme();
	}

	private listenSystemTheme() {
		if (typeof window === 'undefined') return;
		const mq = window.matchMedia('(prefers-color-scheme: dark)');
		this.systemDarkSignal.value = mq.matches;
		mq.addEventListener('change', e => {
			this.systemDarkSignal.value = e.matches;
		});
	}

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

	colorScheme = reactive(async (): Promise<ColorScheme> => {
		const settings = await this.settings();
		if (settings.color_scheme === 'light' || settings.color_scheme === 'dark')
			return settings.color_scheme;
		return 'system';
	});

	qrColor = reactive(async () => {
		const settings = await this.settings();
		return settings.qr_color || DEFAULT_QR_COLOR;
	});

	localMailboxEnabled = reactive(async () => {
		const settings = await this.settings();
		return settings.local_mailbox_enabled;
	});

	notificationsEnabled = reactive(async () => {
		const settings = await this.settings();
		return settings.notifications_enabled;
	});

	isDark = reactive(async () => {
		const systemDark = this.systemDarkSignal.value;
		const scheme = await this.colorScheme();
		if (scheme === 'light') return false;
		if (scheme === 'dark') return true;
		return systemDark;
	});

	async setColorScheme(scheme: ColorScheme): Promise<void> {
		await this.client.setSetting(
			'color_scheme',
			scheme === 'system' ? null : scheme,
		);
	}

	async setQrColor(color: string): Promise<void> {
		await this.client.setSetting('qr_color', color);
	}

	async setLocalMailboxEnabled(enabled: boolean): Promise<void> {
		await this.client.setLocalMailboxEnabled(enabled);
	}

	async setNotificationsEnabled(enabled: boolean): Promise<void> {
		await this.client.setNotificationsEnabled(enabled);
	}
}
