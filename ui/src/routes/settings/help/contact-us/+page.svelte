<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { appLogDir, join } from '@tauri-apps/api/path';
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { showToast } from '$lib/utils/toasts';
	import {
		BlockTitle,
		Button,
		Checkbox,
		Dialog,
		DialogButton,
		List,
		ListInput,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		useTheme,
	} from 'konsta/svelte';

	const theme = $derived(useTheme());

	let message = $state('');
	let reason = $state('');
	let feeling = $state<string | null>(null);
	let includeDebugLog = $state(true);
	let showDebugLogDialog = $state(false);

	const feelings = ['😀', '🙂', '😐', '🙁', '😡'];

	const reasonLabels: Record<string, () => string> = {
		bug: () => m.reasonBugReport(),
		feature: () => m.reasonFeatureRequest(),
		question: () => m.reasonQuestion(),
		other: () => m.reasonOther(),
	};

	async function handleSubmit() {
		const subjectParts: string[] = [];
		if (reason) subjectParts.push(reasonLabels[reason]());
		if (feeling) subjectParts.push(feeling);
		const subject = subjectParts.length > 0
			? `Dash Chat: ${subjectParts.join(' - ')}`
			: 'Dash Chat';

		let attachments: string[] | undefined;
		if (includeDebugLog) {
			const logDir = await appLogDir();
			const logFile = await join(logDir, 'Dash Chat.log');
			attachments = [logFile];
		}

		try {
			await invoke('plugin:mailto|mailto', {
				request: {
					email: 'hello@dashchat.org',
					subject,
					body: message,
					attachments,
				},
			});
		} catch {
			showToast(m.errorUnexpected(), 'unexpected');
		}
	}
</script>

<Page>
	<Navbar title={m.contactUs()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink
				onClick={() => goto('/settings/help')}
				data-testid="contact-us-back"
			/>
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="column center-in-desktop">
			<BlockTitle>{m.contactUs()}</BlockTitle>
			<List strongIos inset={isWideScreen.value || theme === 'ios'} class="!mb-0">
				<ListInput
					type="textarea"
					placeholder={m.tellUsWhatsGoingOn()}
					bind:value={message}
					inputClass="!h-32 resize-none"
					data-testid="contact-us-message-input"
				/>
			</List>

			<BlockTitle>{m.tellUsWhyReachingOut()}</BlockTitle>
			<List strongIos inset={isWideScreen.value || theme === 'ios'} class="!mb-0">
				<ListInput
					type="select"
					bind:value={reason}
					data-testid="contact-us-reason-select"
				>
					{#snippet input()}
						<select bind:value={reason}>
							<option value="" disabled>{m.pleaseSelectAnOption()}</option>
							<option value="bug">{m.reasonBugReport()}</option>
							<option value="feature">{m.reasonFeatureRequest()}</option>
							<option value="question">{m.reasonQuestion()}</option>
							<option value="other">{m.reasonOther()}</option>
						</select>
					{/snippet}
				</ListInput>
			</List>

			<BlockTitle>{m.howDoYouFeel()}</BlockTitle>
			<div class="flex gap-2 px-4 mt-6">
				{#each feelings as emoji}
					<button
						class="emoji-btn"
						class:selected={feeling === emoji}
						onclick={() => (feeling = feeling === emoji ? null : emoji)}
						data-testid="contact-us-feeling-{emoji}"
					>
						<span class="text-3xl">{emoji}</span>
					</button>
				{/each}
			</div>

			<List strongIos inset={isWideScreen.value || theme === 'ios'} class="!mt-4 !mb-0">
				<ListItem
					title={m.includeDebugLog()}
					data-testid="contact-us-include-debug-log"
					onClick={() => (includeDebugLog = !includeDebugLog)}
				>
					{#snippet media()}
						<Checkbox
							checked={includeDebugLog}
							onChange={() => (includeDebugLog = !includeDebugLog)}
						/>
					{/snippet}
					{#snippet after()}
					{/snippet}
				</ListItem>
			</List>
		</div>
	</div>

		<Button
			rounded
			onClick={handleSubmit}
			disabled={!message}
			data-testid="contact-us-next-btn"
			class="fixed-action-btn"
		>
			{m.next()}
		</Button>

	<Dialog
		opened={showDebugLogDialog}
		onBackdropClick={() => (showDebugLogDialog = false)}
	>
		{#snippet title()}
			{m.includeDebugLog()}
		{/snippet}
		<span>{m.debugLogExplanation()}</span>
		{#snippet buttons()}
			<DialogButton onClick={() => (showDebugLogDialog = false)}>
				{m.done()}
			</DialogButton>
		{/snippet}
	</Dialog>
</Page>

<style>
	.emoji-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 48px;
		height: 48px;
		border-radius: 50%;
		border: none;
		background-color: var(--k-color-bg-300, rgba(128, 128, 128, 0.15));
		cursor: pointer;
		-webkit-tap-highlight-color: transparent;
	}

	.emoji-btn.selected {
		background-color: var(--k-color-brand-primary, #007aff);
	}

	.emoji-btn:active {
		opacity: 0.7;
	}

	.whats-this-link {
		background: none;
		border: none;
		color: var(--k-color-brand-primary, #007aff);
		cursor: pointer;
		font-size: 14px;
		padding: 0;
	}

	.faq-link {
		background: none;
		border: none;
		color: var(--k-color-brand-primary, #007aff);
		cursor: pointer;
		font-size: 14px;
		padding: 0;
	}
</style>
