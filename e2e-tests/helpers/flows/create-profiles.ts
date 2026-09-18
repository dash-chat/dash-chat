import type { Agent } from '../../setup/setup-agents';

/** Create a profile on each agent at once, named after its key, leaving each
 *  on the home page. */
export async function createProfiles(
	profiles: Record<string, Agent>,
): Promise<void> {
	await Promise.all(
		Object.entries(profiles).map(([name, agent]) =>
			agent.createProfilePage.createProfile(name),
		),
	);
}
