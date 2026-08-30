<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import ImageInput from '$lib/components/ImageInput.svelte';
	import { compressImage } from '$lib/utils/compress';
	import { sendFeedback, type FeedbackReason } from '$lib/utils/error-report';
	import { showToast } from '$lib/utils/toasts';
	import {
		BlockTitle,
		Checkbox,
		List,
		ListInput,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		useTheme,
	} from 'konsta/svelte';
	import FixedActionButton from '$lib/components/FixedActionButton.svelte';

	const theme = $derived(useTheme());

	// Without SENTRY_DSN there is nowhere for a submission to go.
	const canSend = import.meta.env.VITE_SENTRY_ENABLED;

	let message = $state('');
	let reason = $state<FeedbackReason | undefined>();
	let includeDebugLog = $state(true);
	let screenshot = $state<File | null>(null);
	let sending = $state(false);

	const reasonLabels: Record<string, () => string> = {
		bug: () => m.reasonBugReport(),
		feature: () => m.reasonFeatureRequest(),
		question: () => m.reasonQuestion(),
		feedback: () => m.reasonGeneralFeedback(),
		other: () => m.reasonOther(),
	};

	async function handleSubmit() {
		if (!reason || sending) return;
		sending = true;
		try {
			const outcome = await sendFeedback({
				reason,
				message,
				screenshot: screenshot ? await compressImage(screenshot) : undefined,
				includeLogs: includeDebugLog,
			});
			showToast(outcome === 'queued' ? m.reportQueued() : m.reportSent());
			goto('/settings/help');
		} catch (e) {
			console.error('Error sending feedback', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		} finally {
			sending = false;
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
			<BlockTitle>{m.tellUsWhyReachingOut()}</BlockTitle>
			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mb-0"
			>
				<ListInput
					type="select"
					bind:value={reason}
					data-testid="contact-us-reason-select"
				>
					{#snippet input()}
						<select bind:value={reason}>
							<option value={undefined} disabled>
								{m.pleaseSelectAnOption()}
							</option>
							<option value="bug">{m.reasonBugReport()}</option>
							<option value="feature">{m.reasonFeatureRequest()}</option>
							<option value="question">{m.reasonQuestion()}</option>
							<option value="feedback">{m.reasonGeneralFeedback()}</option>
							<option value="other">{m.reasonOther()}</option>
						</select>
					{/snippet}
				</ListInput>
			</List>

			<BlockTitle>{m.contactUs()}</BlockTitle>
			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mb-0"
			>
				<ListInput
					type="textarea"
					placeholder={m.tellUsWhatsGoingOn()}
					bind:value={message}
					inputClass="!h-32 resize-none"
					maxlength={4096}
					data-testid="contact-us-message-input"
				/>
			</List>

			<BlockTitle>{m.attachScreenshot()}</BlockTitle>
			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mb-0"
			>
				<ImageInput bind:value={screenshot} />
			</List>

			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mt-4 !mb-0"
			>
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
					{#snippet after()}{/snippet}
				</ListItem>
			</List>

			{#if !canSend}
				<p class="px-4 pt-4 text-sm opacity-60">{m.feedbackUnavailable()}</p>
			{/if}
		</div>
	</div>

	<FixedActionButton
		onClick={handleSubmit}
		disabled={!message || !reason || !canSend}
		loading={sending}
		testId="contact-us-send-btn"
	>
		{m.send()}
	</FixedActionButton>
</Page>
