---
name: release
description: Trigger a Display DJ release. Use for any request to release, publish, ship, cut a version, create release artifacts, or run a release workflow.
---

# Release

Release without asking routing or confirmation questions when the requested action is clear.

## Release mode

- Default to **official** for every release request.
- Use **beta** only when the user explicitly says `beta`, `prerelease`, `pre-release`, `test release`, `test build`, or names `release-beta.yml` or `/release-beta`.
- Words such as `release`, `publish`, `ship`, `latest`, `current`, `new version`, or a version number do not imply beta. They always route to official unless accompanied by an explicit beta/test marker.
- Never infer beta from the current branch, commit SHA, dirty state, failed CI, or uncertainty. If beta/test was not explicit, official remains true.

## Release summary and changesets

Before triggering either workflow:

1. Resolve the latest official GitHub release whose `isDraft` and `isPrerelease` values are both false.
2. Read commits from that tag through the release commit.
3. Summarize those changes for the user, grouped by user-visible behavior, fixes, and maintenance when applicable.
4. Continue automatically. Do not ask the user to confirm the summary, mode, or version.

The summary is informational only. Do not create or edit a changeset, changelog file, release body, `.github/release-body-static.md`, or generated notes file. Do not pass `release_notes`, `notes`, `user_notes`, or equivalent custom-note input to a workflow. Leave those inputs unset so the repository workflow generates its changelog from commits since the last official release.

## Official release

1. Verify the current branch is `main`, the worktree is clean, and local `main` is synchronized with `origin/main`.
2. Read the version from root `package.json`, the source of truth. If the user explicitly supplied a version, apply it with `npm version <version> --no-git-tag-version`; otherwise use the version already committed. Run `npm run format` and verify `src-tauri/tauri.conf.json` matches.
3. Run the repository-required release checks. Fix failures before release; do not downgrade to beta.
4. Push committed changes to `main` after `git pull --rebase`.
5. Trigger `release-official.yml` from `main`. Leave `release_notes` unset. Leave `tag` unset unless the user explicitly supplied a version and the workflow requires the matching `v<version>` value.
6. Report the official version, tag, summarized changes, and workflow URL.

## Beta or test release

Only enter this path after an explicit beta/test marker.

1. Resolve the requested SHA, defaulting to current `HEAD` only when no SHA was supplied.
2. Ensure the commit exists on the remote.
3. Trigger `release-beta.yml` from `main`, supplying `sha` only when needed. Leave `notes` unset.
4. Report the SHA, expected beta tag, summarized changes since the last official release, and workflow URL.
