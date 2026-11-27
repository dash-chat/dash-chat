set working-directory := '..'

# regenerate paraglide message exports from source translation files
paraglide:
    pnpm --filter ui paraglide

# The scenes are the scenes.ts files next to the routes. Set CHROME_BIN where
# Chrome is not installed where Playwright looks for it; SCENES=a,b takes only
# those scenes.
#
# take the screenshots translators see, one per scene, failing when a source string has none
screenshots:
    #!/usr/bin/env bash
    set -euo pipefail
    pnpm -F dash-chat-stores -F dash-chat-mock build

    # A copy of the messages and of the inlang project that reads them, where
    # every English message ends in the invisible marker of its key:
    # ui/vite.config.ts has paraglide compile from it for this build.
    rm -rf ui/.marked-messages
    mkdir -p ui/.marked-messages/project.inlang
    cp ui/project.inlang/settings.json ui/.marked-messages/project.inlang/
    # The plugins the project already fetched, which the build would
    # otherwise have to fetch again.
    if [ -d ui/project.inlang/cache ]; then
        cp -r ui/project.inlang/cache ui/.marked-messages/project.inlang/cache
    fi
    cp -r ui/messages ui/.marked-messages/messages
    node scripts/i18n-screenshots/mark-messages.ts ui/messages/en.json ui/.marked-messages/messages/en.json

    # `vite build` alone stops at the staging directory, so the build the app
    # embeds is left as it was. It compiles the marked messages into the
    # sources the UI imports, so the real ones are compiled back however this
    # ends: built, failed or interrupted.
    trap 'pnpm -F ui paraglide' EXIT
    VITE_TRANSLATION_KEY_MARKERS=true pnpm -F ui exec vite build

    # Serve that build for as long as the screenshots take; how they went is
    # how this ends.
    pnpm exec concurrently --kill-others --success first --names preview,capture \
        "pnpm -F ui exec vite preview --port 4173 --strictPort" \
        "node scripts/i18n-screenshots/capture.ts http://localhost:4173"

# Needs WEBLATE_TOKEN; which Weblate component is in
# scripts/i18n-screenshots/sync-weblate.ts.
#
# upload those screenshots to Weblate, attached to the strings each one shows
sync-weblate:
    node scripts/i18n-screenshots/sync-weblate.ts
