import { PermissionDialog } from '../../helpers/components/permission-dialog';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Notification permission', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'android' }]);
		await agent.stopApp();
		await agent.resetNotificationPermission();
		await agent.startApp();
	});

	after(async () => {
		await agent.stopApp();
		await agent.resetNotificationPermission();
	});

	it('turns notifications on when creating a profile', async () => {
		await agent.createProfilePage.createProfile('Nora');
		if (agent.asksNotificationPermission()) {
			await new PermissionDialog(agent).allow();
		}

		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();
		await expect(agent.notificationsPage.toggleInput).toBeSelected();
	});

	it('keeps the toggle off when notifications are denied', async () => {
		await agent.notificationsPage.toggleSwitch.click();
		await expect(agent.notificationsPage.toggleInput).not.toBeSelected();
		await agent.stopApp();
		await agent.denyNotificationPermission();
		await agent.startApp();

		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();
		await agent.notificationsPage.toggleSwitch.click();

		await agent.toast.expectMessage(
			await agent.tr('notificationsPermissionDenied'),
		);
		await expect(agent.notificationsPage.toggleInput).not.toBeSelected();
	});
});
