/** TODO: pre-contact: flip to true once a requester can send messages before
 *  the request is accepted. Every step declared with `itPreAccept` is the spec
 *  for that behavior and runs again; every block gated on
 *  `!PRE_ACCEPT_MESSAGING` is a stopgap for the disabled feature and can then
 *  be deleted. */
export const PRE_ACCEPT_MESSAGING = false;

/** `it` for a step that specifies pre-accept messaging: kept in the spec, and
 *  skipped only while the feature is disabled. */
export const itPreAccept = PRE_ACCEPT_MESSAGING ? it : it.skip;
