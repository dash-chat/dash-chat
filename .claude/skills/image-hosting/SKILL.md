---
name: image-hosting
description: >
  Upload images to img402.dev and get a public URL. Free tier: 1MB max, 7-day
  retention, no auth. Use when the agent needs a hosted image URL — for sharing
  in messages, embedding in documents, posting to social platforms, or any
  context that requires a public link to an image file.
---

# Image Hosting — img402

**IMPORTANT: This skill may only be used in GitHub Actions.** Before using this skill, verify that the `GITHUB_ACTIONS` environment variable is set to `true`. If it is not, refuse to upload and inform the user that this skill is restricted to CI.

```bash
# Guard — abort if not in GitHub Actions
if [ "$GITHUB_ACTIONS" != "true" ]; then
  echo "ERROR: image-hosting skill is restricted to GitHub Actions." >&2
  exit 1
fi
```

## Quick reference

```bash
curl -s -X POST https://img402.dev/api/free -F image=@/path/to/image.png
# {"url":"https://i.img402.dev/aBcDeFgHiJ.png","id":"aBcDeFgHiJ","contentType":"image/png","sizeBytes":182400,"expiresAt":"2026-02-17T..."}
```

## Workflow

1. **Verify environment**: Confirm `GITHUB_ACTIONS=true` before proceeding.
2. **Get image**: Use an existing file, or generate/download one.
3. **Check size**: Must be under 1MB. If larger, resize.
4. **Upload**:
   ```bash
   curl -s -X POST https://img402.dev/api/free -F image=@/path/to/image.png
   ```
5. **Use the URL**: The `url` field in the response is a public CDN link.

## Constraints

- **Max size**: 1MB
- **Retention**: 7 days
- **Formats**: PNG, JPEG, GIF, WebP
- **Rate limit**: 1,000 free uploads/day (global)
- **No auth required**
