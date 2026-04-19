---
name: release-official
description: Trigger the official release workflow. Shows changelog since last tag, accepts optional notes, and watches the run.
user_invocable: true
---

# Release Official

Trigger the official release workflow (`release-official.yml`).

## Steps

1. Run `gh run list --workflow=release-official.yml --limit=1 --json status,conclusion,databaseId` to check if there is already a release in progress. If a run is in progress, warn the user and stop.

2. Gather the release notes context:
   - Run `git fetch --tags` to ensure tags are up to date.
   - Find the last official release tag: `git tag --sort=-v:refname | grep -v "^release-beta-" | head -n 1`
   - Show the user the commits since that tag: `git log --oneline <prev_tag>..HEAD`
   - Ask the user if they want to add optional release notes, or proceed with auto-generated notes only.

3. Trigger the workflow:
   - If the user provided notes: `gh workflow run release-official.yml -f release_notes="<user notes>"`
   - If no notes: `gh workflow run release-official.yml`

4. Wait a few seconds, then get the run ID: `gh run list --workflow=release-official.yml --limit=1 --json databaseId -q '.[0].databaseId'`

5. Stream the workflow output: `gh run watch <run_id>` (use `run_in_background` so it doesn't block).

6. Once complete, show the user the result and link to the release page.
