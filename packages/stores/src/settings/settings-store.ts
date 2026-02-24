import { reactive, signal } from 'signalium';

import type { ISettingsClient } from './settings-client.js';

export type ColorScheme = 'light' | 'dark' | 'system';

export class SettingsStore {
	private settingsVersion = signal(0);
	private systemDarkSignal = signal<boolean>(false);

	constructor(public client: ISettingsClient) {
		this.listenSystemTheme();
	}

	private listenSystemTheme() {
		if (typeof window === 'undefined') return;
		const mq = window.matchMedia('(prefers-color-scheme: dark)');
		this.systemDarkSignal.value = mq.matches;
		mq.addEventListener('change', (e) => {
			this.systemDarkSignal.value = e.matches;
		});
	}

	private settings = reactive(async () => {
		this.settingsVersion.value;
		return this.client.getSettings();
	});

	colorScheme = reactive(async (): Promise<ColorScheme> => {
		const settings = await this.settings();
		if (settings.color_scheme === 'light' || settings.color_scheme === 'dark')
			return settings.color_scheme;
		return 'system';
	});

	qrColor = reactive(async () => {
		const settings = await this.settings();
		return settings.qr_color || '#007aff';
	});

	isDark = reactive(async () => {
		const systemDark = this.systemDarkSignal.value;
		const scheme = await this.colorScheme();
		if (scheme === 'light') return false;
		if (scheme === 'dark') return true;
		return systemDark;
	});

	async setColorScheme(scheme: ColorScheme): Promise<void> {
		await this.client.setSetting('color_scheme', scheme === 'system' ? null : scheme);
		this.settingsVersion.value++;
	}

	async setQrColor(color: string): Promise<void> {
		await this.client.setSetting('qr_color', color);
		this.settingsVersion.value++;
	}
}
