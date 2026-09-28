import { PermissionDialog } from '../../helpers/components/permission-dialog';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Notifications settings follow the OS permission', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'android' }]);
		await agent.createProfilePage.createProfile('Nora');
	});

	it('shows notifications on while the permission is granted', async () => {
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();

		await expect(agent.notificationsPage.toggleInput).toBeSelected();
		await expect(agent.notificationsPage.toggleInput).toBeEnabled();
		await expect(agent.notificationsPage.permissionBanner).not.toBeDisplayed();
	});

	it('asks for the permission from the banner while it was never requested', async () => {
		await agent.stopApp();
		agent.resetNotificationPermission();
		await agent.startApp();

		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();

		await expect(agent.notificationsPage.permissionBanner).toBeDisplayed();
		await expect(agent.notificationsPage.toggleInput).not.toBeSelected();
		await expect(agent.notificationsPage.toggleInput).toBeDisabled();

		await agent.notificationsPage.turnOnButton.click();
		await new PermissionDialog(agent).allow();

		await expect(agent.notificationsPage.toggleInput).toBeSelected();
		await expect(agent.notificationsPage.permissionBanner).not.toBeDisplayed();
	});

	it('sends the banner to the system settings once the permission is denied for good', async () => {
		await agent.stopApp();
		agent.resetNotificationPermission();
		await agent.startApp();

		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();

		const dialog = new PermissionDialog(agent);
		await agent.notificationsPage.turnOnButton.click();
		await dialog.deny();
		await agent.notificationsPage.turnOnButton.click();
		await dialog.deny();

		await agent.notificationsPage.turnOnButton.click();
		await expect(agent.notificationsPage.settingsSheet).toBeDisplayed();
		await expect(agent.notificationsPage.toggleInput).toBeDisabled();
	});
});
