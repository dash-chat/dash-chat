# Admins

Taking Signal's semantics,

*Admin* is a special status that a group member can have.
The creator of a group always begins as an Admin.

A group has a set of *Permissions*, which determines what additional abilities normal members (non-Admins) have in the group.
Only an Admin can change the Permissions, and Permissions never limit the abilities of any Admin.

The Permissions we'll implement are:
- Add (but not remove) members from the group
- Edit group info

Even with full Permission, there are still things which only Admins can do:

- Remove a member (except that members can always remove themselves)
- Change the Admin status of any member

## p2panda implementation

These Permissions are more fine-grained than p2panda-auth's model.
The distinction we care about there is between: 

- Write access, which only allows sending messages (including things like changing group info)
- Manage access, which grants ability to add/remove and change Admin status of other members

The way p2panda-spaces enforces its own access controls is by each honest node dropping operations which would contradict the current member state.
For instance, any non-Admin can forge an operation which adds an outsider to the group, but every honest node's p2panda-auth processor will ignore that operation, causing every node to agree on an encryption state which excludes that outside.

Since p2panda-auth has coarser access levels than our permissions, we need to hook into the p2panda pipeline with our own application logic that selectively rejects operations that aren't permitted, like a member trying to remove another member.

As of today, there is no way to do this.
We need something like custom processors, so we can define a processor that sits before auth, so forbidden operations never make it into auth/space state.

### "Admin" vs "Manager"

Note that because we want members to have a Permission to add new members, and the only p2panda access level that allows that is Manager, we can't rely on p2panda's access levels to denote Admin status. 
We have to represent it explicitly.

We also have to do one of the following:

1. make every member a Manager, and then subtract abilities according to the Permissions
2. every time we change this Permission, we change the access level of every non-Admin between Write and Manage
3. we change p2panda to have a more fine-grained access model

(1) probably makes the most sense. (2) only solves the problem of toggling ability to add members, but we still have to subtract remove/promote/demote, so we might as well rely completely on subtraction. (3) would be great but we all have to agree.

# Custom validation

Enforcing our permissions model requires something like an extension of p2panda-auth's DAG. 
Our operations that change permissions are equally important as the auth operations which change access levels. 
We have to handle concurrent changes to permissions, and have every node agree on who had which permissions when.

