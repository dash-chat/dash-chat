# Messaging a group member

Members of groups who are not already contacts can message each other.
This requires sending a ContactRequest to that fellow member.

The usual flow begins with a QR code scan, which has an Inbox topic associated with it.
In this case, there is no such code or inbox.
The only shared Space between you is any groups you are both in.
(Previously, the Announcements topic would have been a good place for this, but that had to be removed in the p2panda-spaces refactor.)

One place to put the ContactRequest is in the group itself.
It can be encrypted toward the target, without a recipient listed, so everyone will have to trial decrypt it.

This is a bit of a leak when there is only one or a small number of members in a group, such that others may be able to deduce the target just by the existence of an encrypted contact request, but this can be communicated.
If privacy is needed, the contact request can be made in the usual way, by asking a mutual contact to pass on a request link/QR code to the target, so that only that mutual contact need be trusted.

## New `Sealed` operation for groups

An operation that's intended for a particular member can be encrypted with a key in their current Member keybundle, which they should be able to decrypt. TODO: find out the details of how p2panda does this.

