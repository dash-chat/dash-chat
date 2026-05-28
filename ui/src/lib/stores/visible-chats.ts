import type { ChatsStore } from 'dash-chat-stores';
import { derived } from 'svelte/store';

import { previewFeatures } from './preview-features.svelte';
import { useReactivePromise } from './use-signal';

export function useVisibleChatSummaries(chatsStore: ChatsStore) {
	const all = useReactivePromise(chatsStore.allChatsSummaries);
	return derived(all, $all =>
		$all.then(summaries =>
			summaries.filter(s => s.type !== 'GroupChat' || previewFeatures.enabled),
		),
	);
}
