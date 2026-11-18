# Disappearing Messages

Chat members with the correct permissions can set Disappearing Messages to an arbitrary time interval (at least 28 days).

```rust
ChatPayload {
    /// Published to the "current" log (TODO what does that mean?) as well as to the new log. 
    /// Must be same author for both, and must be the first operation on the new log.
    SetDisappearingMessages {
        /// If None, turns off disappearing messages
        interval: Some(Duration),
    }
}
```

## Processing

Anyone who processes this op subscribes to this new log and publishes new messages there.
The node also maintains a mapping of `(ChatId, LogId) -> Option<Duration>` to keep track of the disappearing status for a given log in a space.
TODO: how is the "active" log tracked?

~~When the creator of the op processes it, the creator also re-publishes it to the new log as the first message, if that log has not already been created.~~
This new log is governed by the time interval, and every node will prune the log accordingly. ~~**except** for the first message which defines the interval, which should never be pruned!~~
The LogId is derived from the non-disappearing LogId with interval mixed in, to deterministically produce the LogId governed by a particular setting.
This is so that if two nodes independently choose the same disappearing setting, they will converge on the same LogId.

## Partitions

Since members may be partitioned, there is no guarantee that a member will see another's Disappearing Messages setting, and there may even be two different concurrent settings, resulting in two different logs.
Assuming that we implement this feature with log pruning, and that p2panda will implement a way for operations to have a time-triggered PruneFlag or similar, then messages with two different Disappearing settings are incompatible, because we have to prune all messages before a certain point in the log.
This is why we must switch to a different log every time we change the setting.

### Conflict resolution

If two settings were chosen concurrently, say 1 day and 1 week, then there should be a tiebreaker, so that everyone at one setting moves over to the other one.
Without a tiebreaker, the chat would be split across two logs at different disappearing intervals, which is not a problem per se, but it's a design decision to keep it unified whenever possible.
One natural choice would be to choose the shorter interval.

When the members on the longer interval discover the active log with the shorter interval, they immediately switch over to the new log.
Their messages on the longer interval remain on the old log and will disappear at the original interval.

Some UX considerations:

- If the change to disappearing messages is shown as an "event" in the chat, then it's not always the case that every message following that event has the same disappearing interval.
- The "event" is something that happened retroactively from the perspective of the longer interval members, but it does not *apply* retroactively to their messages. Somehow this needs to be conveyed.

## Active Log

Now that there can be multiple concurrent logs for chat messages, we need to define which one is active at a given time.

- All chat spaces start on a single message log, which starts out Active.
- Any time we process a SetDisappearingMessages op on the active log,
  - if its interval is shorter than any other op already processed on the same log, then switch to that log
- TODO


## Mailboxes

Mailboxes must be aware of the per-log disappearing setting so it can also prune.

