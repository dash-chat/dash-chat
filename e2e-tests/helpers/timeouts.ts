/** Normal wait for things that happen automatically: a page loading, a button
 *  enabling, your own message rendering locally. This is the WDIO global
 *  `waitforTimeout`, so it applies to every wait that doesn't ask for more. */
export const UI_TIMEOUT = 10_000;

/** Much longer wait for a message to propagate peer-to-peer — sent on one
 *  agent, rendered on another. p2p sync through the mailbox is slow to settle
 *  on cold, headless CI runners, so anything cross-agent waits this long. */
export const SYNC_TIMEOUT = 60_000;
