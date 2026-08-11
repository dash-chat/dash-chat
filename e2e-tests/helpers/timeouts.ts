/** Normal wait for things that happen automatically: a page loading, a button
 *  enabling, your own message rendering locally. This is the WDIO global
 *  `waitforTimeout`, so it applies to every wait that doesn't ask for more.
 *  Generous enough for slow Android emulators. */
export const UI_TIMEOUT = 30_000;

/** Much longer wait for a message to propagate peer-to-peer — sent on one
 *  agent, rendered on another. p2p sync through the mailbox is slow to settle
 *  on cold, headless CI runners, so anything cross-agent waits this long. */
export const SYNC_TIMEOUT = 60_000;

/** Cross-agent wait for media attachments: the bytes ride the iroh blob
 *  channel (with a mailbox relay hop) separately from the message op, which
 *  is slower still than op sync on cold CI runners. */
export const MEDIA_SYNC_TIMEOUT = 120_000;

/** How long something already on screen must survive to count as untouched by
 *  an arriving message. One arrival re-renders a chat more than once: the
 *  message itself, then the read receipt its appearance publishes on a 500ms
 *  debounce, then the render that receipt's own operation triggers. Asserting
 *  before the whole burst has landed passes on a chat that is about to tear
 *  the element down. */
export const RENDER_SETTLE_WINDOW = 5_000;
