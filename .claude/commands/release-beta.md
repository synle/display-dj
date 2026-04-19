---
description: Trigger a beta prerelease build from the current HEAD (or a specific SHA)
allowed-tools: [Bash, Read]
---

# Beta Release

Trigger a beta prerelease build of Display DJ.

## Instructions

1. **Get the current commit**:
   ```bash
   git rev-parse --short HEAD
   ```

2. **Push any unpushed commits** (check if ahead of remote):
   ```bash
   git status -sb
   ```
   If ahead, push first:
   ```bash
   git push origin main
   ```

3. **Trigger the beta workflow**:
   ```bash
   gh workflow run release-beta.yml --ref main
   ```

4. **Report** the short SHA, expected beta tag (`release-beta-<date>-<sha>`), and build URL to the user.
