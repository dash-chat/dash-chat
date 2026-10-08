import { PermissionDialog } from '../../helpers/components/permission-dialog';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Notifications after allowing them during profile creation', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'android' }]);
		await agent.stopApp();
		agent.resetNotificationPermission();
		await agent.startApp();
	});

	it('turns notifications on once the permission is allowed', async () => {
		await agent.createProfilePage.createProfile('Nora');
		await new PermissionDialog(agent).allow();

		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();
		await expect(agent.notificationsPage.toggleInput).toBeSelected();
	});

	it('shows notifications off while the permission is revoked', async () => {
		await agent.stopApp();
		agent.denyNotificationPermission();
		await agent.startApp();

		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();
		await expect(agent.notificationsPage.toggleInput).not.toBeSelected();
	});

	it('keeps the toggle off when the permission is denied for good, and offers the settings', async () => {
		await agent.notificationsPage.toggleSwitch.click();
		await agent.waitUntil(() => agent.notificationsPage.isSettingsSheetOpen());
		await expect(agent.notificationsPage.toggleInput).not.toBeSelected();
	});

	it('shows notifications on again once the permission is allowed again', async () => {
		await agent.stopApp();
		agent.grantNotificationPermission();
		await agent.startApp();

		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();
		await expect(agent.notificationsPage.toggleInput).toBeSelected();
	});
});
