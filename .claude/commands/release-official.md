---
description: Create an official release without confirmation prompts; summarize commits since the last official release and let the workflow generate release notes
allowed-tools: [Bash, Read, Edit, Glob, Grep]
---

# Official Release

Create a published official release of Display DJ. This is the default mode for every release request unless the user explicitly requests beta, prerelease, or test output.

## Instructions

1. Verify `main` is clean and synchronized with `origin/main`. Fix release-blocking validation failures instead of switching release modes.

2. Find the last non-draft, non-prerelease GitHub release:

   ```bash
   gh release list --limit 100 --json tagName,isDraft,isPrerelease --jq '[.[] | select(.isDraft == false and .isPrerelease == false)] | .[0].tagName'
   ```

3. Read commits from that tag through `HEAD`, then summarize the changes for the user:

   ```bash
   git log --oneline --no-decorate <last_tag>..HEAD
   ```

   Continue automatically. Do not ask the user to confirm the summary, official mode, or version.

4. Read the version from root `package.json`, the source of truth. If the user explicitly supplied a version, apply it with `npm version <version> --no-git-tag-version`. Otherwise use the version already committed. Run `npm run format` and verify `src-tauri/tauri.conf.json` matches.

5. Do not create or edit a changeset, changelog, generated release-notes file, or `.github/release-body-static.md`. Do not set `release_notes`, `notes`, `user_notes`, or any equivalent workflow input. The workflow owns changelog generation from commits since the last official release.

6. Run the repository release checks, then synchronize and push:

   ```bash
   git pull --rebase
   git push origin main
   ```

7. Trigger the official workflow without custom notes:

   ```bash
   gh workflow run release-official.yml --ref main
   ```

   Supply `-f tag=v<version>` only when the user explicitly requested that exact version and it matches both version files. Never supply `release_notes`.

8. Report the official version, expected tag, summarized changes, and workflow URL.
