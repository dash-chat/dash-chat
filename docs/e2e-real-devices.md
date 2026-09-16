# Setting Up Real Devices for E2E Tests

`PLATFORMS` lists the agents to launch (default `desktop,desktop`): `desktop` (Linux only), `android` (USB phone), `android-emulator`, or `ios` (macOS only, USB iPhone).

```bash
PLATFORMS=android,android just e2e run send-messages
PLATFORMS=android,desktop just e2e run send-messages
PLATFORMS=ios,ios just e2e run send-messages
```

Run one suite at a time per host: the devices, the pinned Appium/adb/mailbox ports and the host Wi-Fi card are shared.

## Android phone

1. Enable Developer options and USB debugging, plug the phone in, accept the USB debugging prompt. It must show as `device` in:

   ```bash
   nix develop .#androidDev --command adb devices
   ```

   Restarting the adb daemon drops the authorisation; accept the prompt again.

2. The phone's system WebView major must match one of the chromedrivers pinned as `nixpkgs-chromedriver-*` inputs in `flake.nix` (149 to 152 today). If a WebView update moves it past the set, add a nixpkgs revision that ships that chromedriver.

3. Nothing else: the harness captures the `androidDev` shell env itself, builds the APK for the phone's ABI, installs it, bridges the mailbox with `adb reverse`, and keeps the screen awake (it sets `svc power stayon true` and a 30 minute `screen_off_timeout`, both persistent, so use a dedicated test device).

`adb devices` order decides which phone is agent 1 and agent 2. Pin with `ANDROID_UDID1=<serial>` / `ANDROID_UDID2=<serial>`.

Wi-Fi specs (`local-hub-discovery*`, `p2p-network-switch`) drive the phone with `cmd wifi` over adb, which needs a recent Android release (verified on Android 16).

## iPhone

1. On the Mac: Xcode, and `brew install libimobiledevice` (for `idevice_id` and `idevicesyslog`). Run the suite from a plain shell, not a nix shell.

2. Sign in to Xcode with an Apple ID that is a member of the team in `src-tauri/gen/apple/dash-chat.xcodeproj`. WebDriverAgent is built under that team, and a freshly connected phone is registered on the developer portal during the first WDA build.

3. Plug the phone in, unlock it, trust the Mac. It must appear in `idevice_id -l`.

4. Put the phone on the same LAN as the Mac. The app bakes the Mac's LAN IPv4 as its mailbox address; if the harness picks the wrong interface, set `E2E_HOST_IP`.

5. For Wi-Fi specs, set the phone's language to English: the harness drives the Settings app by its labels.

`idevice_id -l` order decides the agent slots. Pin with `IOS_UDID1` / `IOS_UDID2`. Two-agent specs need two iPhones, since desktop cannot run on the Mac.

## Wi-Fi lab

Specs that move phones and hubs between networks need real access points, listed in a gitignored `e2e-tests/.env` (see `e2e-tests/.env.example`):

```
E2E_WIFI_NETWORKS=dash-lab-a:passphraseA,dash-lab-b:passphraseB
```

Each access point serves DHCP on its own subnet, uses WPA2-PSK or no security, and needs no upstream. The phones and the host start on the same network; the host's Wi-Fi card joins the lab networks during a run (NetworkManager on Linux, `networksetup` on macOS) while the host keeps its wired link. Unset, every network move stays out of the runs.
