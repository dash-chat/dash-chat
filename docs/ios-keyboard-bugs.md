# iOS keyboard handling — state of affairs and proposed fix

## Symptoms

Two open issues track the same family of bugs on iOS:

- **#139** — chat header and input bar collapsed into a thin strip at the top of the screen, the rest is a black void. Sending the first message silently fails; the second send works.
- **#201** — pure black screen with the keyboard up after navigating into a freshly added contact's chat. No header, no input, no way out.

Both occur on iOS (Dynamic Island layout, iPhone status bar).

## Root cause: our custom plugin

`tauri-plugin-virtual-keyboard-padding` (sibling repo) ships an iOS implementation in `ios/Sources/VirtualKeyboardPaddingPlugin.swift`. It removes WKWebView's built-in keyboard observers and replaces them with its own logic that:

1. Resizes the WKWebView frame down by the keyboard height when the keyboard shows.
2. Injects a `<style id="__vkp">` overriding `.min-h-screen` / `.h-screen` to a pixel value (because `100vh` doesn't update on frame resize).
3. Auto-detects the page background color via JS and paints the WebView, its scroll view, every ancestor view, and the window with that color so the keyboard animation doesn't flash white.
4. Swizzles `WKContentView.inputAccessoryView` to drop the "Done" toolbar.
5. Disables scrolling and clamps `contentOffset` to `.zero` via `UIScrollViewDelegate`.

The shape of the approach is correct. The implementation has several races and one-shot decisions that produce exactly the symptoms above.

### Bugs in the plugin

1. **`keyboardWillShow` early-returns when `keyboardHeight != 0`** (line 137). Predictive bar appearing/disappearing, language switch, and autofill all fire `keyboardWillChangeFrame` (which we don't observe at all) followed by another `keyboardWillShow`. The plugin keeps the *old* height, so frame resize and CSS injection use stale values.
2. **`originalHeight = webView.frame.size.height` is captured at show time.** If the WebView frame is mid-animation, was already shrunk by an unbalanced previous show/hide, or has been touched by a safe-area change, `originalHeight` records a too-small value. Then `newHeight = originalHeight − keyboardHeight` becomes tiny or negative. This matches **#139**.
3. **No SPA-navigation awareness.** `#__vkp` and `--keyboard-height` are document-global. SvelteKit navigations between a `keyboardWillShow` and the matching `keyboardWillHide` leave the new page rendered against stale CSS.
4. **`backgroundColorSet` is one-shot** (line 144). The detection runs on the first `keyboardWillShow` and never again. If the focused page hasn't painted an opaque `.k-page` / body background yet (loading state, fresh mount, route transition), the JS walks up parents, returns nothing, and the native side never overrides its default (black). Black then propagates to the WebView, scroll view, every ancestor view, and the window. The next keyboard show on a slow-painting route flashes pure black. This matches **#201**.
5. **`isScrollEnabled = false` plus the `contentOffset = .zero` clamp** means there is no recovery path. Once the frame is wrong the user can't scroll to find content.

## Upstream Tauri / wry status

There is no upstream fix to pull in. iOS keyboard handling is unowned in Tauri/wry as of May 2026:

- [tauri-apps/tauri#9907](https://github.com/tauri-apps/tauri/issues/9907) — "iOS pop-up keyboard causes webview to move outside the screen". **Open**, `needs triage`, no PR.
- [tauri-apps/tauri#10631](https://github.com/tauri-apps/tauri/issues/10631) — "visualViewport API fails to account for mobile keyboard height". **Open**. Comments only contain Android `WindowInsetsCompat` workarounds; nothing for iOS.
- [tauri-apps/tauri#13479](https://github.com/tauri-apps/tauri/issues/13479) — "Window size not adjusting on initial page load". Closed as duplicate of #10631.
- [tauri-apps/wry recent wkwebview commits](https://github.com/tauri-apps/wry/commits/dev/src/wkwebview) — last six months touch background color, multi-window, dylib loading. Nothing keyboard-related.
- Community plugins of similar shape exist ([voxelbee/tauri-plugin-webview-scroll](https://github.com/voxelbee/tauri-plugin-webview-scroll), [tauri-plugin-ios-keyboard](https://crates.io/crates/tauri-plugin-ios-keyboard)) but they are not upstream fixes and have the same general design.

## Two fix paths

### Option A — keep frame resize, fix the bugs

Smallest change. Stays inside our plugin. Patches:

- Drop the `keyboardHeight == 0` guard. Handle `keyboardWillChangeFrame` and recompute on every event.
- Capture the baseline once in `load()` (`webView.superview!.bounds.height` or the safe-area layout frame) instead of reading `webView.frame.size.height` at every show.
- Re-run `detectAndApplyBackgroundColor` until it returns a non-null color, instead of one-shot. Or kick it off on `WKNavigationDelegate.didFinish` so each new route gets a fresh detection.
- Re-inject `#__vkp` on SvelteKit navigation, or keep the style element permanent and just update the CSS variable.
- Keep this approach if we're confident we want full Swift-side control over keyboard behavior.

Unblocks #139 and #201 without changing the architecture.

### Option B — let WKWebView handle the keyboard, use the web platform

iOS Safari 18.4 (April 2025) shipped support for the `interactive-widget` viewport meta value:

```html
<meta name="viewport" content="width=device-width, initial-scale=1, interactive-widget=resizes-content">
```

With `resizes-content`, both the visual and layout viewports shrink when the keyboard appears: `100dvh` updates, `position: fixed; bottom: 0` sits above the keyboard, `env(keyboard-inset-bottom)` reports the right value. This is what the plugin is currently emulating in Swift.

The catch: this only works if WKWebView's native keyboard observers stay in place — i.e. roll back the plugin to:
- Keep the inset-adjustment / "Done" toolbar removal.
- Drop the manual frame resize, the JS injection, and the keyboard observer registration.

Trade-offs:
- Removes ~250 lines of fragile Swift and the entire stale-`originalHeight` / SPA-CSS / black-flash family of bugs.
- Requires verifying every route that has a fixed bottom bar (chat input, action FABs, navbars on iOS theme) still lays out correctly above the autocomplete strip.
- Needs a fallback on iOS ≤ 18.3. That window is shrinking but not yet trivial.

## Recommendation

Do **Option A** now to unblock #139 and #201. Schedule **Option B** as a follow-up once we're willing to set an iOS 18.4 floor or write the fallback, since it removes a meaningful maintenance burden.

## References

- [tauri-apps/tauri#9907](https://github.com/tauri-apps/tauri/issues/9907)
- [tauri-apps/tauri#10631](https://github.com/tauri-apps/tauri/issues/10631)
- [tauri-apps/tauri#13479](https://github.com/tauri-apps/tauri/issues/13479)
- [tauri-apps/tauri Discussion #9368](https://github.com/tauri-apps/tauri/discussions/9368)
- [`interactive-widget` in CSS Viewport spec](https://drafts.csswg.org/css-viewport/#interactive-widget)
- [Safari 18.4 release notes](https://webkit.org/blog/16574/webkit-features-in-safari-18-4/)
- [voxelbee/tauri-plugin-webview-scroll](https://github.com/voxelbee/tauri-plugin-webview-scroll)
- [tauri-plugin-ios-keyboard on crates.io](https://crates.io/crates/tauri-plugin-ios-keyboard)
