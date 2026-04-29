# How to Create a Release

## 1. Create a PR from develop to main

From the commit on `develop` that you want to release, open a pull request targeting `main`.

Wait for all CI checks to pass before proceeding.

## 2. Deploy to production servers

Checkout the release commit locally and merge it into the `production` branch to trigger a Garnix deployment:

```bash
git checkout production
git merge <commit-sha>
git push
```

## 3. Build and test

Build the desktop binary:

```bash
pnpm tauri build
```

Install on Android:

```bash
just android install
```

Install on iOS:

```bash
just ios install
```

Manually test the app on desktop, Android, and iOS.

## 4. Cut the release

Once everything looks good, run the release script:

```bash
just release 0.12.0
```
