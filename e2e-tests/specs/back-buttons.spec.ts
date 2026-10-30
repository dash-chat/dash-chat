import { backToHome } from '../helpers/flows/back-to-home';
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import { expectBackPops } from '../helpers/flows/expect-back-pops';
import { type Agent, setupAgents } from '../setup/setup-agents';

const GROUP_NAME = 'backgroup';

/**
 * Regression net for Back wiring: every Back button must go through
 * `goBack` (pop the history stack), not `goto` to a hardcoded parent
 * (push onto it). `expectBackPops` proves the pop via history.forward().
 */
describe('Back buttons pop the history stack', () => {
	let agent: Agent;
	let peer: Agent;

	before(async function () {
		[agent, peer] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Walker: agent, Peer: peer });
		await exchangeContacts([agent, peer]);
		await backToHome([agent, peer]);
	});

	async function openSettings() {
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
	}

	async function closeSettings() {
		await agent.settingsPage.back.click();
		await agent.homePage.ready();
	}

	async function openProfile() {
		await openSettings();
		await agent.settingsPage.profileLink.click();
		await agent.profilePage.ready();
	}

	async function closeProfile() {
		await agent.profilePage.back.click();
		await agent.settingsPage.ready();
		await closeSettings();
	}

	it('settings pops to home', async () => {
		await expectBackPops(agent, {
			open: () => agent.homePage.settingsLink.click(),
			ready: () => agent.settingsPage.ready(),
			back: () => agent.settingsPage.back.click(),
			parentReady: () => agent.homePage.ready(),
		});
	});

	it('profile pops to settings', async () => {
		await openSettings();
		await expectBackPops(agent, {
			open: () => agent.settingsPage.profileLink.click(),
			ready: () => agent.profilePage.ready(),
			back: () => agent.profilePage.back.click(),
			parentReady: () => agent.settingsPage.ready(),
		});
		await closeSettings();
	});

	it('edit-name pops to profile', async () => {
		await openProfile();
		await expectBackPops(agent, {
			open: () => agent.profilePage.editName.click(),
			ready: () => agent.editNamePage.ready(),
			back: () => agent.editNamePage.back.click(),
			parentReady: () => agent.profilePage.ready(),
		});
		await closeProfile();
	});

	it('edit-about pops to profile', async () => {
		await openProfile();
		await expectBackPops(agent, {
			open: () => agent.profilePage.editAbout.click(),
			ready: () => agent.editAboutPage.ready(),
			back: () => agent.editAboutPage.back.click(),
			parentReady: () => agent.profilePage.ready(),
		});
		await closeProfile();
	});

	it('edit-photo close pops to profile', async () => {
		await openProfile();
		await expectBackPops(agent, {
			open: () => agent.profilePage.editPhoto.click(),
			ready: () => agent.editPhotoPage.ready(),
			back: () => agent.editPhotoPage.close.click(),
			parentReady: () => agent.profilePage.ready(),
		});
		await closeProfile();
	});

	it('profile add-contact pops to profile', async () => {
		await openProfile();
		await expectBackPops(agent, {
			open: () => agent.profilePage.qrLink.click(),
			ready: () => agent.addContactPage.ready(),
			back: () => agent.addContactPage.back.click(),
			parentReady: () => agent.profilePage.ready(),
		});
		await closeProfile();
	});

	it('appearance pops to settings', async () => {
		await openSettings();
		await expectBackPops(agent, {
			open: () => agent.settingsPage.appearanceLink.click(),
			ready: () => agent.appearancePage.ready(),
			back: () => agent.appearancePage.back.click(),
			parentReady: () => agent.settingsPage.ready(),
		});
		await closeSettings();
	});

	it('notifications pops to settings', async () => {
		await openSettings();
		await expectBackPops(agent, {
			open: () => agent.settingsPage.notificationsLink.click(),
			ready: () => agent.notificationsPage.ready(),
			back: () => agent.notificationsPage.back.click(),
			parentReady: () => agent.settingsPage.ready(),
		});
		await closeSettings();
	});

	it('account pops to settings', async () => {
		await openSettings();
		await expectBackPops(agent, {
			open: () => agent.settingsPage.accountLink.click(),
			ready: () => agent.accountPage.ready(),
			back: () => agent.accountPage.back.click(),
			parentReady: () => agent.settingsPage.ready(),
		});
		await closeSettings();
	});

	it('offline pops to settings', async function () {
		// The offline settings page exists everywhere but iOS ({#if !isIos}).
		if (agent.platform === 'ios') this.skip();
		await openSettings();
		await expectBackPops(agent, {
			open: () => agent.settingsPage.offlineLink.click(),
			ready: () => agent.offlinePage.ready(),
			back: () => agent.offlinePage.back.click(),
			parentReady: () => agent.settingsPage.ready(),
		});
		await closeSettings();
	});

	it('help pops to settings', async () => {
		await openSettings();
		await expectBackPops(agent, {
			open: () => agent.settingsPage.helpLink.click(),
			ready: () => agent.helpPage.ready(),
			back: () => agent.helpPage.back.click(),
			parentReady: () => agent.settingsPage.ready(),
		});
		await closeSettings();
	});

	it('contact-us pops to help', async () => {
		await openSettings();
		await agent.settingsPage.helpLink.click();
		await agent.helpPage.ready();
		await expectBackPops(agent, {
			open: () => agent.helpPage.contactUsLink.click(),
			ready: () => agent.contactUsPage.ready(),
			back: () => agent.contactUsPage.back.click(),
			parentReady: () => agent.helpPage.ready(),
		});
		await agent.helpPage.back.click();
		await agent.settingsPage.ready();
		await closeSettings();
	});

	it('feedback opened from home pops back to home', async () => {
		await expectBackPops(agent, {
			open: () => agent.testBanner.feedbackButton.click(),
			ready: () => agent.contactUsPage.ready(),
			back: () => agent.contactUsPage.back.click(),
			parentReady: () => agent.homePage.ready(),
		});
	});

	it('new-message pops to home', async () => {
		await expectBackPops(agent, {
			open: () => agent.homePage.newMessageButton.click(),
			ready: () => agent.newMessagePage.ready(),
			back: () => agent.newMessagePage.back.click(),
			parentReady: () => agent.homePage.ready(),
		});
	});

	it('new-message add-contact pops to new-message', async () => {
		await agent.homePage.newMessageButton.click();
		await agent.newMessagePage.ready();
		await expectBackPops(agent, {
			open: () => agent.newMessagePage.addContact.click(),
			ready: () => agent.addContactPage.ready(),
			back: () => agent.addContactPage.back.click(),
			parentReady: () => agent.newMessagePage.ready(),
		});
		await agent.newMessagePage.back.click();
		await agent.homePage.ready();
	});

	it('direct chat pops to home', async () => {
		await expectBackPops(agent, {
			open: () => agent.homePage.openChat('Peer'),
			ready: () => agent.directChatPage.ready(),
			back: () => agent.directChatPage.back.click(),
			parentReady: () => agent.homePage.ready(),
		});
	});

	it('feedback opened from a chat pops back to that chat', async () => {
		await agent.homePage.openChat('Peer');
		await agent.directChatPage.ready();
		await expectBackPops(agent, {
			open: () => agent.testBanner.feedbackButton.click(),
			ready: () => agent.contactUsPage.ready(),
			back: () => agent.contactUsPage.back.click(),
			parentReady: () => agent.directChatPage.ready(),
		});
		await agent.directChatPage.back.click();
		await agent.homePage.ready();
	});

	it('chat-settings pops to the chat', async () => {
		await agent.homePage.openChat('Peer');
		await agent.directChatPage.ready();
		await expectBackPops(agent, {
			open: () => agent.directChatPage.settingsLink.click(),
			ready: () => agent.chatSettingsPage.ready(),
			back: () => agent.chatSettingsPage.back.click(),
			parentReady: () => agent.directChatPage.ready(),
		});
		await agent.directChatPage.back.click();
		await agent.homePage.ready();
	});

	it('creating a group replaces the wizard: Back lands on new-message', async () => {
		await createGroup(agent, GROUP_NAME, ['Peer']);
		await agent.groupChatPage.ready();
		await agent.groupChatPage.back.click();
		await agent.newMessagePage.ready();
		expect(await agent.execute(() => window.__test.currentPath())).toEqual(
			'/new-message',
		);
		await agent.newMessagePage.back.click();
		await agent.homePage.ready();
	});

	it('group chat pops to home', async () => {
		await expectBackPops(agent, {
			open: () => agent.homePage.chatListItem(GROUP_NAME).click(),
			ready: () => agent.groupChatPage.ready(),
			back: () => agent.groupChatPage.back.click(),
			parentReady: () => agent.homePage.ready(),
		});
	});

	it('group info pops to the group chat', async () => {
		await agent.homePage.chatListItem(GROUP_NAME).click();
		await agent.groupChatPage.ready();
		await expectBackPops(agent, {
			open: () => agent.groupChatPage.infoLink.click(),
			ready: () => agent.groupInfoPage.ready(),
			back: () => agent.groupInfoPage.back.click(),
			parentReady: () => agent.groupChatPage.ready(),
		});
		await agent.groupChatPage.back.click();
		await agent.homePage.ready();
	});

	it('group info edit pops to group info', async () => {
		await agent.homePage.chatListItem(GROUP_NAME).click();
		await agent.groupChatPage.ready();
		await agent.groupChatPage.infoLink.click();
		await agent.groupInfoPage.ready();
		await expectBackPops(agent, {
			open: () => agent.groupInfoPage.editLink.click(),
			ready: () => agent.groupInfoEditPage.ready(),
			back: () => agent.groupInfoEditPage.back.click(),
			parentReady: () => agent.groupInfoPage.ready(),
		});
		await agent.groupInfoPage.back.click();
		await agent.groupChatPage.ready();
		await agent.groupChatPage.back.click();
		await agent.homePage.ready();
	});

	it('add-members pops to group info', async () => {
		await agent.homePage.chatListItem(GROUP_NAME).click();
		await agent.groupChatPage.ready();
		await agent.groupChatPage.infoLink.click();
		await agent.groupInfoPage.ready();
		await expectBackPops(agent, {
			open: () => agent.groupInfoPage.addMembersLink.click(),
			ready: () => agent.addMembersPage.ready(),
			back: () => agent.addMembersPage.back.click(),
			parentReady: () => agent.groupInfoPage.ready(),
		});
		await agent.groupInfoPage.back.click();
		await agent.groupChatPage.ready();
		await agent.groupChatPage.back.click();
		await agent.homePage.ready();
	});
});
