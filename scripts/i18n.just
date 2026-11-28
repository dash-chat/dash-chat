set working-directory := '..'

# regenerate paraglide message exports from source translation files
paraglide:
    pnpm --filter ui paraglide

# The scenes are the scenes.ts files next to the routes, run by Playwright,
# which what follows the recipe's name is passed to: --ui to watch them play,
# --debug to step through one. Set CHROME_BIN where Chrome is not installed
# where Playwright looks for it; SCENES=a,b takes only those scenes. With
# I18N_SCREENSHOTS_SYNC=1 it then uploads them to Weblate, attached to the
# strings each one shows, as the token in WEBLATE_TOKEN: it syncs what it just
# took, and only when it took every scene.
#
# take the screenshots translators see, one per scene, failing when a source string has none
screenshots *playwright_args:
    #!/usr/bin/env bash
    set -euo pipefail
    pnpm -F dash-chat-stores build

    # A copy of the messages and of the inlang project that reads them, where
    # every English message ends in the invisible marker of its key:
    # ui/vite.config.ts has paraglide compile from it for this build.
    rm -rf ui/.marked-messages
    mkdir ui/.marked-messages
    cp -r ui/project.inlang ui/messages ui/.marked-messages/
    node scripts/i18n-screenshots/messages.ts ui/messages/en.json ui/.marked-messages/messages/en.json

    # `vite build` alone stops at the staging directory, so the build the app
    # embeds is left as it was. It compiles the marked messages into the
    # sources the UI imports, so the real ones are compiled back however this
    # ends: built, failed or interrupted.
    trap 'pnpm -F ui paraglide' EXIT
    VITE_TRANSLATION_KEY_MARKERS=true pnpm -F ui exec vite build

    pnpm exec playwright test --config scripts/i18n-screenshots {{playwright_args}}
