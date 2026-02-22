import { invoke } from '@tauri-apps/api/core';

export interface IPreferencesClient {
    fetchState(): Promise<any>
    setState(preferences: Record<string,any>): Promise<void>
}

export class PreferencesClient implements IPreferencesClient {
    async fetchState(): Promise<any> {
        console.log('client get prefs');
        return await invoke('get_preferences');
    }
    async setState(preferences: Record<string,any>): Promise<void> {
        console.log('client setting prefs', preferences)
        return await invoke('set_preferences', {preferences: preferences})
    }
}
