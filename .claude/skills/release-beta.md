---
name: release-beta
description: Trigger the beta release workflow. Lets you pick a SHA and notes, and watches the run.
user_invocable: true
---

# Release Beta

Trigger the beta release workflow (`release-beta.yml`). Creates a draft prerelease tagged `release-beta-<date>-<sha>`.

## Steps

1. Run `gh run list --workflow=release-beta.yml --limit=1 --json status,conclusion,databaseId` to check if there is already a beta release in progress. If a run is in progress, warn the user and stop.

2. Gather context:
   - Run `git fetch --tags` to ensure tags are up to date.
   - Find the last official release tag: `git tag --sort=-v:refname | grep -v "^release-beta-" | head -n 1`
   - Show the user the commits since that tag: `git log --oneline <prev_tag>..HEAD`
   - Ask the user which SHA to build from (defaults to HEAD if empty), and if they want to add optional release notes.

3. Trigger the workflow:
   - `gh workflow run release-beta.yml -f sha="<sha>" -f notes="<user notes>"`
   - Omit `-f sha` or pass empty string to use HEAD.
   - Omit `-f notes` or pass empty string for no notes.

4. Wait a few seconds, then get the run ID: `gh run list --workflow=release-beta.yml --limit=1 --json databaseId -q '.[0].databaseId'`

5. Stream the workflow output: `gh run watch <run_id>` (use `run_in_background` so it doesn't block).

6. Once complete, show the user the result and link to the draft release page.
