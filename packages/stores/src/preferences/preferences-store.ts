import { reactive, Signal, signal } from 'signalium';

import { IPreferencesClient } from './preferences-client';

export interface PreferencesData {
    appearanceTheme: 'system' | 'dark' | 'light'
}

export class PreferencesStore {
    constructor(
        public client: IPreferencesClient
    ) {
        this.fetch()
    }

    public preferences = reactive(async () => {
        let defaulted:PreferencesData = {
            appearanceTheme: this.innerState.value.appearanceTheme ?? 'system'
        }
        return defaulted
    })

    innerState:Signal<Record<string, any>> = signal({})

    async fetch () {
        console.log('fetching state for store')
        let result = await this.client.fetchState()
        if(typeof result === 'object') {
            this.innerState.update(() => result)
        }
    }


    public async setTheme(value: PreferencesData['appearanceTheme']) {
        console.log('store set them', value)
        let result = {
            ...this.innerState.value,
            appearanceTheme: value
        }
        await this.client.setState(result)
        await this.fetch()
    }
}