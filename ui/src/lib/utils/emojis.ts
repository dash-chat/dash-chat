import type { DeviceId } from "dash-chat-stores";

export interface CondensedReaction {
    emoji: string;
    count: number;
    own: boolean;
}

export function condenseReactions(reactions: Record<DeviceId, string>, ownDeviceId: DeviceId): Array<CondensedReaction> {
    const mapping = new Map<string, CondensedReaction>()
    Object.entries(reactions).map(([device, emoji]) => {
        let entry = mapping.get(emoji)
        if (entry) {
            entry.count = entry.count +  1
            entry.own = device === ownDeviceId ? true : entry.own
        } else {
            mapping.set(emoji, { emoji: emoji, own: device === ownDeviceId, count: 1 })
        }
    })
    return Array.from(mapping.values())
}