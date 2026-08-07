<script lang="ts">
	import { Navbar, NavbarBackLink, Page } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';

	let { onBack }: { onBack: () => void } = $props();
</script>

<Page>
	<Navbar
		title={m.termsAndPrivacyPolicy()}
		titleClass="opacity1"
		transparent={true}
	>
		{#snippet left()}
			<NavbarBackLink onClick={onBack} data-testid="eula-back" />
		{/snippet}
	</Navbar>

	<div class="column center-in-desktop px-4 pb-12" data-testid="eula-body">
		<h1 class="mb-1 text-xl font-bold">
			Dash Chat Messenger — End User Licence Agreement
		</h1>
		<p class="mb-6 text-sm opacity-60">Last updated: 2026-08-07</p>

		<p class="mb-3 text-sm leading-relaxed">
			This End User Licence Agreement ("Agreement") is between you and Darksoil
			Studio Limited, a company registered in England and Wales (company number
			15640125) ("Darksoil", "we", "us"). It governs your use of the Dash Chat
			Messenger application ("Dash Chat") and any server infrastructure we
			operate in connection with it.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			By installing or using Dash Chat, you agree to this Agreement. If you do
			not agree, do not install or use Dash Chat.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">1. Acceptable use</h2>
		<p class="mb-3 text-sm leading-relaxed">
			You agree to use Dash Chat only for legal, authorised, and acceptable
			purposes. You will not use Dash Chat, or assist others in using Dash Chat,
			in ways that:
		</p>
		<ul class="mb-3 list-disc ps-5 text-sm leading-relaxed">
			<li class="mb-1">
				violate or infringe the rights of Darksoil, our users, or others,
				including privacy, publicity, intellectual property, or other
				proprietary rights;
			</li>
			<li class="mb-1">
				involve sending illegal or impermissible communications, such as bulk
				messaging, automated messaging, or unsolicited messages.
			</li>
		</ul>
		<p class="mb-3 text-sm leading-relaxed">
			You must not (or assist others to) access, use, modify, distribute,
			transfer, or exploit Dash Chat or our server infrastructure in
			unauthorised manners, or in ways that harm Darksoil, Dash Chat, or our
			systems. (Modifying and distributing the source code as permitted by the
			AGPL-3.0 is authorised — see Section 10.) For example, you must not:
		</p>
		<ul class="mb-3 list-disc ps-5 text-sm leading-relaxed">
			<li class="mb-1">
				gain or try to gain unauthorised access to our systems;
			</li>
			<li class="mb-1">
				disrupt the integrity or performance of Dash Chat or any system we
				operate;
			</li>
			<li class="mb-1">
				create accounts through unauthorised or automated means;
			</li>
			<li class="mb-1">
				collect information about our users in any unauthorised manner;
			</li>
			<li class="mb-1">
				sell, rent, or charge for access to server infrastructure we operate.
			</li>
		</ul>

		<h2 class="mb-2 mt-6 text-base font-bold">2. Enforcement</h2>
		<p class="mb-3 text-sm leading-relaxed">
			We have no obligation to monitor, review, or screen any content
			transmitted through Dash Chat.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			We may, at our sole discretion and without notice or liability, restrict,
			rate-limit, suspend, or terminate any account's access to server
			infrastructure we operate — in response to a report, if you violate the
			letter or spirit of this Agreement, or if your use creates harm, risk, or
			possible legal exposure for Darksoil or our users. If we suspend or
			terminate an account for a violation of this Agreement, you agree not to
			create another account without our permission.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			We are under no obligation to take any particular action in any case. Any
			decision not to act does not waive our right to act in any other case, and
			does not create any expectation or entitlement.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">3. What we can and cannot do</h2>
		<ul class="mb-3 list-disc ps-5 text-sm leading-relaxed">
			<li class="mb-1">
				You may block another user or remove them as a contact.
			</li>
			<li class="mb-1">
				We may restrict or suspend an account's access to server infrastructure
				we operate.
			</li>
			<li class="mb-1">
				We cannot remove a message from another person's device once delivered.
				Each user controls the content stored on their own device.
			</li>
		</ul>
		<p class="mb-3 text-sm leading-relaxed">
			Dash Chat is designed to operate across multiple transports, including
			peer-to-peer and local network connections that do not pass through
			infrastructure we operate. Where communication does not use our
			infrastructure, we have no ability to act, and any controls available to
			you are those within the application.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">4. Contact and blocking</h2>
		<p class="mb-3 text-sm leading-relaxed">
			Dash Chat is designed so that you generally receive messages only from
			people you have added as a contact and whose request you have accepted.
			You may remove a contact or block a user at any time. We do not warrant
			that these controls will prevent all unwanted contact in all
			circumstances.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">5. Reporting</h2>
		<p class="mb-3 text-sm leading-relaxed">
			You may report a user from within the application. A report sends us the
			reported account's identifier and a reference to the reported message. It
			does not send us the content of your messages. We may use reports to
			identify accounts engaged in abusive or automated behaviour. We do not
			undertake to investigate, respond to, or act on any individual report.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">6. Your responsibilities</h2>
		<p class="mb-3 text-sm leading-relaxed">
			Your message history is stored on your own devices, not by us. You are
			responsible for keeping your device, your account, and any recovery
			credentials safe and secure, and for the security of the content stored on
			your devices. We are not responsible for any loss of data, loss of access,
			or disclosure of content resulting from loss, theft, seizure, or
			compromise of a device, or from your own configuration choices.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			You are responsible for data and carrier fees and taxes associated with
			the devices and networks on which you use Dash Chat.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">
			7. No access to emergency services
		</h2>
		<p class="mb-3 text-sm leading-relaxed">
			Dash Chat does not provide access to emergency services such as police,
			fire services, ambulance, or other public safety organisations. Make sure
			you can contact emergency services through a mobile or fixed-line
			telephone, or another service.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">8. Availability and updates</h2>
		<p class="mb-3 text-sm leading-relaxed">
			Dash Chat and any server infrastructure we operate may be unavailable,
			interrupted, degraded, or discontinued at any time, with or without
			notice, including during network disruption, maintenance, or equipment
			failure. We may discontinue any feature, platform, or service at any time.
			We do not warrant that Dash Chat will be available or functional in any
			particular circumstance.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			To enable new features and maintain security, you consent to downloading
			and installing updates to Dash Chat.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">9. Eligibility</h2>
		<p class="mb-3 text-sm leading-relaxed">
			You must be at least 13 years old to use Dash Chat. The minimum age to use
			Dash Chat without parental approval may be higher in your country.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">
			10. Software licence and intellectual property
		</h2>
		<p class="mb-3 text-sm leading-relaxed">
			The Dash Chat source code is free and open source software, licensed under
			the GNU Affero General Public License version 3 (AGPL-3.0). Nothing in
			this Agreement limits, restricts, or overrides any right granted to you
			under that licence in respect of the source code. Where this Agreement and
			the AGPL-3.0 conflict in respect of the source code, the AGPL-3.0
			prevails.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			This Agreement governs your use of the compiled application distributed
			through the Apple App Store and your use of server infrastructure operated
			by Darksoil. Subject to your compliance with this Agreement, we grant you
			a limited, revocable, non-exclusive, non-transferable licence to use that
			application on any Apple-branded product that you own or control, as
			permitted by the Usage Rules in the Apple Media Services Terms and
			Conditions.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			You retain any rights you hold in the content you create or send through
			Dash Chat. We claim no ownership of it.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			The Dash Chat and Darksoil names, logos, and trade dress are our property
			and are not licensed under the AGPL-3.0. You may not use them without our
			written permission, except as permitted by applicable law.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">11. Third-party services</h2>
		<p class="mb-3 text-sm leading-relaxed">
			Dash Chat may allow you to access or interact with third-party websites,
			applications, content, networks, or services. Their own terms and privacy
			policies govern your use of them. We are not responsible for third-party
			services.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">12. Privacy</h2>
		<p class="mb-3 text-sm leading-relaxed">
			Our handling of personal data — including what we can and cannot access —
			is described in our Privacy Policy at https://dashchat.org/privacy.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">13. No warranty</h2>
		<p class="mb-3 text-sm leading-relaxed">
			YOU USE DASH CHAT AT YOUR OWN RISK AND SUBJECT TO THE FOLLOWING
			DISCLAIMERS. DASH CHAT IS PROVIDED "AS IS" AND "AS AVAILABLE", WITHOUT
			WARRANTY OF ANY KIND, WHETHER EXPRESS, IMPLIED, OR STATUTORY, INCLUDING
			WITHOUT LIMITATION ANY IMPLIED WARRANTY OF MERCHANTABILITY, FITNESS FOR A
			PARTICULAR PURPOSE, TITLE, NON-INFRINGEMENT, OR FREEDOM FROM HARMFUL CODE.
			WE DO NOT WARRANT THAT DASH CHAT WILL BE UNINTERRUPTED, SECURE,
			ERROR-FREE, ACCURATE, OR AVAILABLE IN ANY PARTICULAR CIRCUMSTANCE. WE DO
			NOT CONTROL AND ARE NOT RESPONSIBLE FOR HOW OR WHEN USERS USE DASH CHAT,
			OR FOR THE ACTIONS, CONTENT, OR INFORMATION OF USERS OR THIRD PARTIES.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			TO THE FULLEST EXTENT PERMITTED BY LAW, YOU RELEASE DARKSOIL, ITS
			DIRECTORS, OFFICERS, EMPLOYEES, PARTNERS, AND AGENTS FROM ANY CLAIM OR
			DAMAGES, KNOWN OR UNKNOWN, RELATING TO OR ARISING OUT OF ANY CLAIM YOU
			HAVE AGAINST ANY THIRD PARTY.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			You are responsible for assessing whether Dash Chat is appropriate for
			your situation and risk profile.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">14. Limitation of liability</h2>
		<p class="mb-3 text-sm leading-relaxed">
			TO THE FULLEST EXTENT PERMITTED BY LAW, DARKSOIL SHALL NOT BE LIABLE FOR
			ANY INDIRECT, INCIDENTAL, SPECIAL, CONSEQUENTIAL, PUNITIVE, OR EXEMPLARY
			DAMAGES, OR FOR ANY LOSS OF DATA, PROFITS, BUSINESS, OR GOODWILL, ARISING
			OUT OF OR IN ANY WAY CONNECTED WITH THIS AGREEMENT OR YOUR USE OF DASH
			CHAT, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGES. OUR TOTAL
			AGGREGATE LIABILITY ARISING OUT OF OR IN CONNECTION WITH THIS AGREEMENT
			SHALL NOT EXCEED SEVENTY-FIVE POUNDS (£75).
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			Nothing in this Agreement — including Sections 13 and 14 — excludes or
			limits liability for death or personal injury caused by negligence, for
			fraud or fraudulent misrepresentation, or for any other liability that
			cannot lawfully be excluded or limited under the law of England and Wales.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">15. Termination</h2>
		<p class="mb-3 text-sm leading-relaxed">
			You may end this Agreement at any time by deleting Dash Chat from your
			device and discontinuing use. We may suspend or terminate your access to
			server infrastructure we operate as described in Section 2. Sections 3, 6,
			7, 8, 10, 13, 14, 15, 17, and 18 survive termination.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">16. Changes to this Agreement</h2>
		<p class="mb-3 text-sm leading-relaxed">
			We may update this Agreement from time to time. Where changes are
			material, we will provide notice within the application or through our
			website. Your continued use of Dash Chat confirms your acceptance of the
			updated Agreement, which supersedes prior versions.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">17. General</h2>
		<p class="mb-3 text-sm leading-relaxed">
			If we fail to enforce any provision of this Agreement, that is not a
			waiver of our right to enforce it. If any provision is held unlawful,
			void, or unenforceable, it shall be severed and the remaining provisions
			shall remain in effect. This Agreement, together with our Privacy Policy,
			constitutes the entire agreement between you and Darksoil regarding your
			use of the application and our server infrastructure. You will comply with
			all applicable export control and trade sanctions laws. We reserve the
			right to limit availability of Dash Chat in any country.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">
			18. Governing law and jurisdiction
		</h2>
		<p class="mb-3 text-sm leading-relaxed">
			This Agreement is governed by the law of England and Wales, and the courts
			of England and Wales shall have exclusive jurisdiction over any dispute
			arising out of or in connection with it. Nothing in this section deprives
			you of any protection, or any right to bring or defend proceedings in the
			courts of your country of residence, granted by mandatory consumer
			protection law that cannot be varied by agreement.
		</p>

		<h2 class="mb-2 mt-6 text-base font-bold">19. Contact</h2>
		<p class="mb-1 text-sm">Darksoil Studio Limited</p>
		<p class="mb-1 text-sm">First Floor Secure House, Lulworth Close</p>
		<p class="mb-1 text-sm">C/O Xeinadin Chandler's Ford</p>
		<p class="mb-1 text-sm">Eastleigh, SO53 3TL, United Kingdom</p>
		<p class="mb-1 text-sm">support@dashchat.org</p>

		<h2 class="mb-2 mt-6 text-base font-bold">Schedule 1 — Apple App Store</h2>
		<p class="mb-3 text-sm leading-relaxed">
			The following additional terms apply when Dash Chat is distributed through
			the Apple App Store.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Acknowledgement.</strong> This Agreement is between you and Darksoil
			only, and not with Apple. Darksoil, not Apple, is solely responsible for Dash
			Chat and its content.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Scope of licence.</strong> The licence granted to you for Dash Chat
			is limited to a non-transferable licence to use Dash Chat on any Apple-branded
			product that you own or control, as permitted by the Usage Rules in the Apple
			Media Services Terms and Conditions.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Maintenance and support.</strong> Darksoil is solely responsible for
			providing any maintenance and support services for Dash Chat. Apple has no
			obligation to furnish any maintenance and support services.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Warranty.</strong> Darksoil is solely responsible for any product warranties,
			whether express or implied by law, to the extent not effectively disclaimed.
			In the event of any failure of Dash Chat to conform to any applicable warranty,
			you may notify Apple, and Apple will refund the purchase price (if any) for
			Dash Chat. To the maximum extent permitted by applicable law, Apple will have
			no other warranty obligation whatsoever with respect to Dash Chat.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Product claims.</strong> Darksoil, not Apple, is responsible for addressing
			any claims relating to Dash Chat or your possession and use of it, including
			product liability claims, any claim that Dash Chat fails to conform to any
			applicable legal or regulatory requirement, and claims arising under consumer
			protection, privacy, or similar legislation.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Intellectual property rights.</strong> In the event of any third-party
			claim that Dash Chat or your possession and use of it infringes that third
			party's intellectual property rights, Darksoil, not Apple, will be solely responsible
			for the investigation, defence, settlement, and discharge of any such claim.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Legal compliance.</strong> You represent and warrant that you are not
			located in a country that is subject to a U.S. Government embargo or that has
			been designated by the U.S. Government as a "terrorist supporting" country,
			and that you are not listed on any U.S. Government list of prohibited or restricted
			parties.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Developer name and address.</strong> Questions, complaints, or claims
			regarding Dash Chat should be directed to the contact details in Section 19.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Third party terms.</strong> You must comply with applicable third-party
			terms of agreement when using Dash Chat.
		</p>
		<p class="mb-3 text-sm leading-relaxed">
			<strong>Third party beneficiary.</strong> Apple and Apple's subsidiaries are
			third-party beneficiaries of this Agreement, and upon your acceptance of it,
			Apple will have the right (and will be deemed to have accepted the right) to
			enforce this Agreement against you as a third-party beneficiary.
		</p>
	</div>
</Page>
