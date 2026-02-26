import { reactive, Signal, signal } from 'signalium';
import { IPreferencesClient } from './preferences-client';

export interface PreferencesData {
    appearanceTheme: 'system' | 'dark' | 'light',
    appearanceLanguage: string
}

export class PreferencesStore {
    constructor(
        public client: IPreferencesClient,
        public defaultLocale: string,
        // locale is any because we dont have access to locales types in store
        public setLocale: (locale:any) => void
    ) {
        this.fetch()
    }

    public preferences = reactive(async () => {
        let defaulted: PreferencesData = {
            appearanceTheme: this.innerState.value.appearanceTheme ?? 'system',
            appearanceLanguage: this.innerState.value.appearanceLanguage ?? this.defaultLocale
        }
        return defaulted
    })

    innerState: Signal<Record<string, any>> = signal({})

    async fetch() {
        let result = await this.client.fetchState()
        if (typeof result === 'object') {
            this.innerState.update(() => result)
            if(result.appearanceLanguage) {
                this.setLocale(result.appearanceLanguage)
            }
        }
    }

    public async setTheme(value: PreferencesData['appearanceTheme']) {
        let result = {
            ...this.innerState.value,
            appearanceTheme: value
        }
        await this.client.setState(result)
        await this.fetch()
    }

    public async setLanguage(value: PreferencesData['appearanceLanguage']) {
        let result = {
            ...this.innerState.value,
            appearanceLanguage: value
        }
        await this.client.setState(result)
        await this.fetch()
    }
}