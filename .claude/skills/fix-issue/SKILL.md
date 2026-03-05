---
name: fix-issue
description: >
  Pick up a GitHub issue, understand it, implement the fix, verify with the running app,
  and open a PR with a task summary and screenshots of any UI changes.
user-invocable: true
allowed-tools: mcp__tauri__driver_session, mcp__tauri__webview_screenshot, mcp__tauri__webview_dom_snapshot, mcp__tauri__webview_find_element, mcp__tauri__webview_execute_js, mcp__tauri__webview_get_styles, mcp__tauri__read_logs, mcp__tauri__manage_window, mcp__tauri__ipc_execute_command, mcp__tauri__ipc_monitor, mcp__tauri__ipc_get_captured
---

# Fix GitHub Issue

End-to-end workflow: read a GitHub issue, implement the fix, verify it in the running app, and open a PR.

## Input

The user provides a GitHub issue number or URL. Extract the issue number and repository.
If no repository is specified, default to `dash-chat/dash-chat`.

## Step 1: Read the issue

```bash
gh issue view <number> --repo <repo>
```

Read the full issue body, title, labels, and any comments. Understand:
- What is the expected behavior?
- What is the actual behavior?
- Are there reproduction steps?
- Are there screenshots or references to specific pages/components?

## Step 2: Clarify ambiguities

If anything is unclear about the requirements or implementation approach, use `AskUserQuestion` to ask the user **before** writing any code. Examples of things to clarify:
- Ambiguous acceptance criteria
- Multiple valid implementation approaches
- Missing context about intended behavior
- Whether the fix should also cover edge cases not mentioned in the issue

Do NOT ask if the issue is perfectly clear. Move straight to planning.

## Step 3: Plan and implement

1. Use `EnterPlanMode` to explore the codebase and design the implementation.
2. After the plan is approved, implement the fix.
3. Keep changes minimal and focused — fix only what the issue describes.
4. Run `cargo test` and `pnpm -C ui check` to verify no regressions.

## Step 4: Verify with the running app

**REQUIRED for all changes, especially UI changes.**

1. Invoke the **start-dev** skill to start the development environment.
2. Connect via `driver_session` and use Tauri MCP tools to verify the fix works.
3. For UI changes:
   - Take screenshots of the affected screens **after** the fix.
   - Verify layout, spacing, alignment, text, colors, and interactive states.
   - Save screenshots to `/tmp/` for later upload.
4. When done verifying, stop the driver session and kill dev processes via `TaskStop`.

## Step 5: Create the PR

1. Create a new branch from `develop`:
   ```bash
   git checkout -b fix/<short-description> develop
   ```
2. Commit all changes with a clear message referencing the issue:
   ```
   Fix #<number>: <short description>
   ```
3. Push the branch and create a PR:
   ```bash
   gh pr create --repo <repo> --base develop --title "Fix #<number>: <title>" --body "$(cat <<'EOF'
   ## Summary

   Closes #<number>

   <1-3 bullet points describing what was done>

   ## Screenshots

   <If UI changes were made, embed screenshots here using the github-image-hosting skill>

   ## Test plan

   - [ ] <verification steps>

   🤖 Generated with [Claude Code](https://claude.com/claude-code)
   EOF
   )"
   ```

### Screenshots in PRs

If UI changes were made and screenshots were captured in Step 4:

1. Use the **github-image-hosting** skill to upload each screenshot to img402.dev.
2. Embed the returned URLs in the PR body's Screenshots section.
3. If there are before/after comparisons, use a markdown table:
   ```markdown
   | Before | After |
   |--------|-------|
   | ![before](url) | ![after](url) |
   ```

**Note:** The github-image-hosting skill only works in GitHub Actions. When running locally, attach screenshots manually or mention them in the PR description.

## Step 6: Return the PR URL

Print the PR URL so the user can review it.
