---
name: github-image-hosting
description: >
  Upload images to img402.dev for embedding in GitHub PRs, issues, and comments.
  Images under 1MB are uploaded free (no payment, no auth) and persist for 7 days.
  Use when the agent needs to share an image in a GitHub context — screenshots, mockups,
  diagrams, or any visual.
---

# Image Upload for GitHub

**IMPORTANT: This skill may only be used in GitHub Actions.** Before using this skill, verify that the `GITHUB_ACTIONS` environment variable is set to `true`. If it is not, refuse to upload and inform the user that this skill is restricted to CI.

```bash
# Guard — abort if not in GitHub Actions
if [ "$GITHUB_ACTIONS" != "true" ]; then
  echo "ERROR: github-image-hosting skill is restricted to GitHub Actions." >&2
  exit 1
fi
```

## Quick reference

```bash
curl -s -X POST https://img402.dev/api/free -F image=@/tmp/screenshot.png
# {"url":"https://i.img402.dev/aBcDeFgHiJ.png","id":"aBcDeFgHiJ","contentType":"image/png","sizeBytes":182400,"expiresAt":"2026-02-17T..."}
```

## Workflow

1. **Verify environment**: Confirm `GITHUB_ACTIONS=true` before proceeding.
2. **Get image**: Use an existing file or capture a screenshot.
3. **Check size**: Must be under 1MB. If larger, resize.
4. **Upload**:
   ```bash
   curl -s -X POST https://img402.dev/api/free -F image=@/tmp/screenshot.png
   ```
5. **Embed** the returned `url` in GitHub markdown:
   ```markdown
   ![Screenshot description](https://i.img402.dev/aBcDeFgHiJ.png)
   ```

## GitHub integration

```bash
# Add as PR comment
gh pr comment --body "![Screenshot](https://i.img402.dev/aBcDeFgHiJ.png)"

# Add to issue
gh issue comment 123 --body "![Screenshot](https://i.img402.dev/aBcDeFgHiJ.png)"
```

## Constraints

- **Max size**: 1MB
- **Retention**: 7 days — suitable for PR reviews, not permanent docs
- **Formats**: PNG, JPEG, GIF, WebP
- **Rate limit**: 1,000 free uploads/day (global)
- **No auth required**
