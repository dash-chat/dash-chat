import type { AgentId } from 'dash-chat-stores';

export const QUICK_EMOJIS = ['❤️', '👍', '👎', '😂', '😮', '😢'];

export interface CondensedReaction {
	emoji: string;
	count: number;
	own: boolean;
}

export function condenseReactions(
	reactions: Record<AgentId, string>,
	ownAgentId: AgentId,
): Array<CondensedReaction> {
	const mapping = new Map<string, CondensedReaction>();
	Object.entries(reactions).forEach(([agent, emoji]) => {
		let entry = mapping.get(emoji);
		if (entry) {
			entry.count = entry.count + 1;
			entry.own = agent === ownAgentId ? true : entry.own;
		} else {
			mapping.set(emoji, {
				emoji: emoji,
				own: agent === ownAgentId,
				count: 1,
			});
		}
	});
	return Array.from(mapping.values());
}
