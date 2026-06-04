import type { ChatsStore } from 'dash-chat-stores';
import { derived } from 'svelte/store';

import { previewFeaturesEnabled } from './preview-features.svelte';
import { useReactivePromise } from './use-signal';

export function useVisibleChatSummaries(chatsStore: ChatsStore) {
	const all = useReactivePromise(chatsStore.allChatsSummaries);
	return derived(
		[all, previewFeaturesEnabled],
		([$all, $previewEnabled]) => $all,
	);
}
