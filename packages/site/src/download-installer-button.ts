import { mdiDownload } from '@mdi/js';
import '@shoelace-style/shoelace/dist/components/button/button.js';
import '@shoelace-style/shoelace/dist/components/copy-button/copy-button.js';
import '@shoelace-style/shoelace/dist/components/dropdown/dropdown.js';
import '@shoelace-style/shoelace/dist/components/icon-button/icon-button.js';
import '@shoelace-style/shoelace/dist/components/icon/icon.js';
import '@shoelace-style/shoelace/dist/components/input/input.js';
import '@shoelace-style/shoelace/dist/components/menu-item/menu-item.js';
import '@shoelace-style/shoelace/dist/components/menu/menu.js';
import { wrapPathInSvg } from '@darksoil-studio/holochain-elements';
import { LitElement, css, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

type BrowserType = 'Windows' | 'MacOS Intel' | 'MacOS Silicon' | 'Linux';

const allBrowsersTypes: BrowserType[] = [
	'Windows',
	'MacOS Intel',
	'MacOS Silicon',
	'Linux',
];

function browserType(): BrowserType {
	const ua = navigator.userAgent.toLowerCase();

	if (ua.includes('windows')) return 'Windows';
	if (ua.includes('mac')) {
		if (ua.includes('intel')) return 'MacOS Intel';
		else return 'MacOS Silicon';
	}
	return 'Linux';
}
function download(url: string) {
	const a = document.createElement('a');
	a.href = url;
	a.download = url.split('/').pop()!;
	document.body.appendChild(a);
	a.click();
	document.body.removeChild(a);
}

@customElement('download-installer-button')
export class DownloadInstallerButton extends LitElement {
	@property({ attribute: 'linux-url' })
	linuxUrl: string | undefined;

	@property({ attribute: 'nix-command' })
	nixCommand: string | undefined;

	@property({ attribute: 'macos-intel-url' })
	macosIntelUrl: string | undefined;

	@property({ attribute: 'macos-silicon-url' })
	macosSiliconUrl: string | undefined;

	@property({ attribute: 'windows-url' })
	windowsUrl: string | undefined;

	getUrlFor(browser: BrowserType) {
		switch (browser) {
			case 'Windows':
				return this.windowsUrl;
			case 'MacOS Intel':
				return this.macosIntelUrl;
			case 'MacOS Silicon':
				return this.macosSiliconUrl;
			case 'Linux':
				return this.linuxUrl;
		}
	}

	render() {
		const browser = browserType();
		return html`<div
			style="display:flex; flex-direction: column; gap: 24px; min-width: 12em"
		>
			<div class="button-row" style="display:flex; flex-direction: row;">
				<sl-button
					href="${this.getUrlFor(browser)}"
					variant="primary"
					size="large"
					class="no-border-radius-right"
				>
					<sl-icon slot="prefix" .src=${wrapPathInSvg(mdiDownload)}></sl-icon>
					<span class="full-label">Download for ${browser}</span>
					<span class="short-label">Download</span></sl-button
				>
				<sl-dropdown
					><sl-button
						slot="trigger"
						caret
						variant="primary"
						size="large"
						class="no-border-radius-left"
						style="height: 48px"
					>
					</sl-button>
					<sl-menu>
						${allBrowsersTypes
							.filter(b => b !== browser)
							.map(
								browser => html`
									<sl-menu-item
										@click=${() => download(this.getUrlFor(browser)!)}
									>
										<sl-icon
											slot="prefix"
											.src=${wrapPathInSvg(mdiDownload)}
										></sl-icon>
										${browser}</sl-menu-item
									>
								`,
							)}
					</sl-menu>
				</sl-dropdown>
			</div>

			${this.nixCommand && browser === 'Linux'
				? html`
						<span style="align-self: center"
							>Or, if you have nix installed:</span
						>
						<sl-input .value=${this.nixCommand} style="flex: 1">
							<sl-copy-button
								slot="suffix"
								.value=${this.nixCommand}
							></sl-copy-button>
						</sl-input>
					`
				: html``}
		</div>`;
	}

	static styles = css`
		sl-button.no-border-radius-right {
			flex: 1;
		}
		sl-button.no-border-radius-right::part(base) {
			border-top-right-radius: 0;
			border-bottom-right-radius: 0;
		}
		sl-button.no-border-radius-left::part(base) {
			border-top-left-radius: 0;
			border-bottom-left-radius: 0;
		}
		.short-label {
			display: none;
		}
		@media (max-width: 400px) {
			.full-label {
				display: none;
			}
			.short-label {
				display: inline;
			}
			.button-row {
				justify-content: center;
			}
			sl-button.no-border-radius-right {
				flex: 0 1 auto;
			}
		}
		sl-input::part(input) {
			pointer-events: none;
		}
	`;
}
