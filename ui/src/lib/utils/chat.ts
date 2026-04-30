/** Distance from the bottom (in px) below which we consider the user
 * "at the bottom" of the chat — controls when the scroll-to-bottom
 * button hides and when self-sends snap back to the bottom.
 *
 * Shared between the chat page and its E2E test helpers so both stay
 * in sync if the value is tuned. */
export const SCROLL_BOTTOM_THRESHOLD = 200;
