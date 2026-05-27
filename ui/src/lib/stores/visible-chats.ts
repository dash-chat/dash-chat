import { derived } from 'svelte/store';
import type { ChatsStore } from 'dash-chat-stores';
import { useReactivePromise } from './use-signal';
import { previewFeatures } from './preview-features.svelte';

export function useVisibleChatSummaries(chatsStore: ChatsStore) {
	const all = useReactivePromise(chatsStore.allChatsSummaries);
	return derived(all, $all =>
		$all.then(summaries =>
			summaries.filter(s => s.type !== 'GroupChat' || previewFeatures.enabled),
		),
	);
}
