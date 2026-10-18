---
description: Trigger a beta or test prerelease only when the user explicitly requests beta, prerelease, or test output
allowed-tools: [Bash, Read]
---

# Beta Release

Trigger a beta prerelease build only after an explicit beta, prerelease, or test request. A plain release request always routes to the official release workflow.

## Instructions

1. Confirm the request itself contains an explicit beta/test marker or invoked `/release-beta`. Do not infer beta from branch, SHA, dirty state, failed CI, or uncertainty.

2. Find the last non-draft, non-prerelease GitHub release and summarize commits from that tag through the requested release commit. Continue automatically without asking for confirmation.

3. Resolve the requested commit, defaulting to current `HEAD` only when no SHA was supplied:

   ```bash
   git rev-parse HEAD
   ```

4. Ensure the commit exists on the remote. Push committed changes when needed after the repository-required synchronization and checks.

5. Do not create or edit a changeset, changelog, generated release-notes file, or `.github/release-body-static.md`. Do not set `notes`, `release_notes`, `user_notes`, or any equivalent custom-note input. The workflow owns changelog generation from commits since the last official release.

6. Trigger the beta workflow without custom notes:

   ```bash
   gh workflow run release-beta.yml --ref main
   ```

   Supply `-f sha=<sha>` only when releasing a commit other than the default remote `HEAD`. Never supply `notes`.

7. Report the commit SHA, expected beta tag, summarized changes since the last official release, and workflow URL.
