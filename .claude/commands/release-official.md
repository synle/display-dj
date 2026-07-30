---
description: Create an official release — bump version, generate changelog from last official release, commit, push, and trigger the release-official workflow
allowed-tools: [Bash, Read, Edit, Glob, Grep]
---

# Official Release

Create a published official release of Display DJ.

## Instructions

1. **Find the last official release tag** (non-prerelease, non-draft):

   ```bash
   gh release list --limit 20 --json tagName,isDraft,isPrerelease --jq '[.[] | select(.isDraft == false and .isPrerelease == false)] | .[0].tagName'
   ```

2. **Generate the changelog** from commits since that tag:

   ```bash
   git log --oneline --no-decorate <last_tag>..HEAD
   ```

   Show the changelog to the user and confirm they want to proceed.

3. **Determine the new version**:
   - Read the current version from `src-tauri/tauri.conf.json` → `"version"`
   - Auto-bump the **patch** version (e.g., 6.2.0 → 6.3.0) unless the user specifies otherwise
   - Ask the user to confirm the new version

4. **Bump the version** in `src-tauri/tauri.conf.json`

5. **Commit and push**:

   ```bash
   git add src-tauri/tauri.conf.json
   git commit -m "chore: bump version to <new_version>"
   git push origin main
   ```

6. **Trigger the release workflow**:

   ```bash
   gh workflow run release-official.yml --ref main -f tag=v<new_version>
   ```

7. **Report** the release tag, version, and build URL to the user.
