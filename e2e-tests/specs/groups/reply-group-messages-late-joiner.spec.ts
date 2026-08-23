import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group chat replies across a late joiner', () => {
	let alice: Agent;
	let bobbi: Agent;
	let carol: Agent;

	before(async function () {
		[alice, bobbi, carol] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await alice.enablePreviewFeatures();
		await bobbi.enablePreviewFeatures();
		await carol.enablePreviewFeatures();
		await alice.createProfilePage.createProfile('Alice', 'Test');
		await bobbi.createProfilePage.createProfile('Bobbi', 'Test');
		await carol.createProfilePage.createProfile('Carol', 'Test');

		await exchangeContacts(alice, bobbi);
		await alice.directChatPage.back.click();
		await bobbi.directChatPage.back.click();
		await alice.homePage.ready();
		await bobbi.homePage.ready();

		await exchangeContacts(alice, carol);
		await alice.directChatPage.back.click();
		await carol.directChatPage.back.click();
		await alice.homePage.ready();
		await carol.homePage.ready();

		await createGroup(alice, 'mygroup', 'Bobbi');

		await bobbi.homePage
			.chatListItem('mygroup')
			.waitForExist({ timeout: SYNC_TIMEOUT });
		await bobbi.homePage.chatListItem('mygroup').click();
		await bobbi.groupChatPage.ready();
	});

	it('never shows a late joiner an incorrect reply quote after she catches up on crossing replies', async () => {
		// Alice and bobbi build a chain of replies that crosses each other's
		// logs — mirrors
		// crates/dashchat-node/tests/reply_messages.rs::late_joiner_syncing_crossing_replies_can_hit_target_not_found —
		// before carol ever joins the group.
		await alice.groupChatPage.composer.sendMessage('hello');
		const aliceMsg = await bobbi.groupChatPage.messages.waitForMessage('hello');

		await aliceMsg.reply('hi back');
		const bobbiReply =
			await alice.groupChatPage.messages.waitForMessage('hi back');
		await bobbiReply.waitForReplyQuote('hello');

		await bobbiReply.reply('no you');
		const aliceReply =
			await bobbi.groupChatPage.messages.waitForMessage('no you');
		await aliceReply.waitForReplyQuote('hi back');

		// Carol joins only after both crossing replies already exist in the
		// group's history, so catching her up requires processing alice's and
		// bobbi's logs together.
		await alice.groupChatPage.infoLink.click();
		await alice.groupInfoPage.ready();
		await alice.groupInfoPage.addMembersLink.click();
		await alice.addMembersPage.ready();
		await alice.addMembersPage.addContactByName('Carol');
		await alice.addMembersPage.addButton.click();
		await alice.groupInfoPage.ready();
		await alice.groupInfoPage.back.click();
		await alice.groupChatPage.ready();

		await carol.homePage
			.chatListItem('mygroup')
			.waitForExist({ timeout: SYNC_TIMEOUT });
		await carol.homePage.chatListItem('mygroup').click();
		await carol.groupChatPage.ready();

		const carolBobbiReply =
			await carol.groupChatPage.messages.waitForMessage('hi back');
		const carolAliceReply =
			await carol.groupChatPage.messages.waitForMessage('no you');

		// The known cross-log-ordering race (see the XXX on
		// ReplyError::TargetNotFound) can make carol transiently drop a quote
		// while she's still catching up, but she must never render one
		// pointing at the wrong message once she has.
		await carolBobbiReply.waitForReplyQuote('hello');
		await carolAliceReply.waitForReplyQuote('hi back');
	});
});
