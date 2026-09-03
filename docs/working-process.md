# Working process

How we take a feature from idea to merged PR. The goal is code that integrates
with what exists, matches Signal where Signal is the reference, and reads like
it was written by someone who knows this codebase. This is a living document —
when a PR slips past one of these steps, refine the step.

## 1. Research before writing anything

Three places, in order:

1. **This repo.** Grep for the domain words (e.g. `avatar`, `color`,
   `palette`) and read the components neighboring the one you'll touch. If an
   equivalent system already exists, the task is integration, not invention.
2. **The Signal reference.** Check `signal-reference/` screenshots for the
   affected screens (all three platforms when available).
3. **Signal's source.** For behavior questions screenshots can't answer
   (conventions, algorithms, edge cases), read Signal-Android/Desktop on
   GitHub and cite the file (e.g. `NameUtil.kt` for initials rules). Don't
   guess at conventions or invent values — every constant should trace to
   existing code or the reference.

Deliverable: a 3–5 line **prior-art summary** — what already exists, what
will be reused, what's genuinely new. It goes in the PR description.

## 2. Decide: integrate or extend

- A new palette, token set, helper, or component is forbidden when an
  equivalent exists. Promote the existing one to a shared location instead.
- If the existing thing is the wrong shape, refactor it first in its own
  commit, then build on it ("make the change easy, then make the easy
  change").
- One logical change per branch and PR, based on `develop`.

## 3. Implement to house style

- Follow CLAUDE.md: comment policy (almost none), logical CSS properties,
  Tailwind over custom CSS, flat code with named helpers.
- Mirror neighboring naming (`message-helpers.ts` → `avatar-helpers.ts`).
- Keep commits small and self-contained; each one compiles and passes
  `pnpm check`. If the branch accumulated dead ends, rebuild it — a reviewer
  should never read add-then-remove churn.

## 4. Verify like a user

- `cd ui && pnpm check` (types + prettier).
- Run the app (`start-dev` skill) and use the affected screens.
- Both light and dark mode, every time theming is touched.
- Both agents, whenever state crosses devices (profiles, colors, messages).
- Compare side-by-side against `signal-reference/` screenshots.
- New behavior ships with an e2e spec; new routes ship with page objects and
  a `visit-all-pages` entry.

## 5. Pre-push slop check

Read the full diff once, asking:

- Did I build a parallel version of something that already existed?
- Does any comment restate the code, narrate the edit, or justify it to a
  reviewer? Delete it.
- Are there dead props, unused exports, or options nothing passes?
- Does any constant exist that didn't come from existing code or the
  reference?
- Is the diff bigger than the task? What can come out?
- Would a reviewer ask "why didn't you use X?" about anything in here?

## 6. PR description

State what changed and why, cite the prior art reused (file paths), and cite
the reference evidence for behavior claims (Signal source file or screenshot).
Note what was verified by hand (modes, devices, flows).
