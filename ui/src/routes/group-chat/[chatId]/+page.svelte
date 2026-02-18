<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import '@awesome.me/webawesome/dist/components/relative-time/relative-time.js';
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { lessThanAMinuteAgo, moreThanAnHourAgo } from '$lib/utils/time';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatsStore, ContactsStore } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSend } from '@mdi/js';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Link,
		Button,
		Card,
		ListInput,
		List,
		Messagebar,
		ToolbarPane,
		Icon,
		useTheme,
	} from 'konsta/svelte';
	import { page } from '$app/state';
	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myAgentId = useReactivePromise(contactsStore.myAgentId);

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);

	const messages = useReactivePromise(store.messages);
	const info = useReactivePromise(store.info);
	const allMembers = useReactivePromise(store.allMembers);
	let messageText = $state('');
	let isClickable = $state(false);
	let inputOpacity = $state(0.3);
	const onMessageTextChange = (e: InputEvent) => {
		messageText = (e.target as HTMLInputElement).value;
		isClickable = messageText.trim().length > 0;
		inputOpacity = messageText ? 1 : 0.3;
	};

	async function sendMessage() {
		const message = messageText;
		if (!message || message.trim() === '') return;

		await store.sendMessage(message);
		messageText = '';
	}
	const theme = $derived(useTheme());
</script>

<Page style={theme === 'material' ? 'height: calc(100vh - 57px)' : ''}>
	<Navbar transparent={true} titleClass="opacity1 w-full" centerTitle={false}>
		{#snippet left()}
			<NavbarBackLink onClick={() => goto('/')}  data-testid="group-chat-back" />
		{/snippet}
		{#snippet title()}
			{#await $info then info}
				<Link
					href={`/group-chat/${chatId}/info`}
					data-testid="group-chat-info-link"
					class="gap-2"
					style="display: flex; justify-content: start; align-items: center;"
				>
					<wa-avatar
						image={info.avatar}
						initials={info.name.slice(0, 2)}
						style="--size: 2.5rem"
					>
					</wa-avatar>
					<span>{info.name}</span>
				</Link>
			{/await}
		{/snippet}
	</Navbar>

	<div class={`column ${theme === 'ios'? 'pb-16':''}`}>
		{#await $allMembers then members}
			<div class="center-in-desktop" style="flex:1">
				<div class="column m-2 gap-2">
					{#await $myAgentId then myActorId}
						{#await $messages then messages}
							{#each messages as message}
								{#if myActorId == message.author}
									<Card raised class="message my-message">
										<div class="row gap-2" style="align-items: end">
											<span>{message.content}</span>

											<div class="dark-quiet text-xs">
												{#if lessThanAMinuteAgo(message.timestamp)}
													<span>{m.now()}</span>
												{:else if moreThanAnHourAgo(message.timestamp)}
													<wa-format-date
														hour="numeric"
														minute="numeric"
														hour-format="24"
														date={new Date(message.timestamp)}
													></wa-format-date>
												{:else}
													<wa-relative-time
														sync
														format="narrow"
														date={new Date(message.timestamp)}
													>
													</wa-relative-time>
												{/if}
											</div>
										</div>
									</Card>
								{:else}
									<div class="row gap-2 m-0">
										<wa-avatar
											image={members[message.author].profile?.avatar}
											initials={members[message.author].profile?.name.slice(
												0,
												2,
											)}
											style="--size: 2.5rem"
										>
										</wa-avatar>
										<Card raised class="message others-message">
											<div class="row gap-2" style="align-items: end">
												<span>{message.content}</span>

												<div class="quiet text-xs">
													{#if lessThanAMinuteAgo(message.timestamp)}
														<span>{m.now()}</span>
													{:else if moreThanAnHourAgo(message.timestamp)}
														<wa-format-date
															hour="numeric"
															minute="numeric"
															hour-format="24"
															date={new Date(message.timestamp)}
														></wa-format-date>
													{:else}
														<wa-relative-time
															sync
															format="narrow"
															date={new Date(message.timestamp)}
														>
														</wa-relative-time>
													{/if}
												</div>
											</div>
										</Card>
									</div>
								{/if}
							{/each}
						{/await}
					{/await}
				</div>
			</div>

			<Messagebar
				placeholder={m.typeMessage()}
				onInput={onMessageTextChange}
				value={messageText}
			>
				{#snippet right()}
					<ToolbarPane class="ios:h-10">
						<Link
							iconOnly
							onClick={() => (isClickable ? sendMessage() : undefined)}
							style="opacity: {inputOpacity}; cursor: {isClickable
								? 'pointer'
								: 'default'}"
						>
							<Icon>
								<wa-icon src={wrapPathInSvg(mdiSend)}> </wa-icon>
							</Icon>
						</Link>
					</ToolbarPane>
				{/snippet}
			</Messagebar>

			<!--			<div
				class="column pr-4 bottom-0 left-0 right-0 fixed bg-white dark:bg-gray-800"
				style="display:none"
			>
				<div class="row gap-1 center-in-desktop" style="align-items: center;">
					<List nested style="flex: 1">
						<ListInput
							type="textarea"
							outline
							bind:value={messageText}
							inputStyle="min-height: 1em; padding: 0; max-height: 4em; resize: none"
							placeholder={m.typeMessage()}
						/>
					</List>

					<Link iconOnly onClick={sendMessage}>
						<wa-icon src={wrapPathInSvg(mdiSend)}> </wa-icon>
					</Link>
				</div>
			</div> -->
		{/await}
	</div>
</Page>
