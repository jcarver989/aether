# Make entity.toString stable for new entities

Currently the output of `a.toString()` on a new entity is `a#1` or `a#2` etc, where the `#n` is based on `a`s index in the list of unsaved/new `Author`s.

However once we `em.flush` and/or assign ids, the output switches to `a:1` / `a:2` / etc, using the assigned id, which is:

- a) basically good, but
- b) means we can't easily which "was this a:2 previously a#2 or a#1"?
- c) if the em is still used, some other new instance might take `a#1`

A more stable algorithm would be:

* We keep a counter of `i` per meta
* Each new `em.create`-d new gets assigned its `i++` (would basically match today's behavior)
* The counters are not reset by `em.flush`, so we'd never see `a#1`-used-again (new change)
* The output of `a.toString()` for new-then-assigned entities would be `a#1:1` i.e. both the "#1 you're a new entity" and the ":1 your assigned id" (new change)

The output of `a#1:1` is definitely "different" so we shouldn't jump in to this, but it seems like once we got used to it, the improved ergonomics of tracking "who exactly is a#1 over time" would probably be worth it.

