#!/bin/sh
# Cut a release in one command. main and v* tags are protected, so it goes
# the way any change does: a PR that must pass the required `ci` check.
#
#   release.sh X.Y.Z [--notes-file FILE] [--no-push]
#       1. Cut (or update) branch release-X.Y.Z from origin/main in a scratch
#          worktree: set plugin.cfg's version to X.Y.Z, turn CHANGELOG.md's
#          Unreleased section into X.Y.Z (FILE replaces it instead), commit
#          "release X.Y.Z", push, open the release PR.
#       2. Wait for the PR's required checks, then merge it.
#       3. Tag the merge commit vX.Y.Z and push the tag (as you: only
#          repository admins may create v* tags).
#       4. The tag runs release.yml, which builds all three
#          binaries and publishes the GitHub release with
#          GdPlanningAI-X.Y.Z.zip attached. Wait for it and print its URL.
#       Rerunning resumes: an open PR is updated, a merged release is tagged,
#       a pushed tag is watched. --no-push stops after the commit and keeps
#       the worktree for inspection.
#
# RELEASE_BASE=<ref> cuts the release commit from that ref instead of origin/main, for a
# --no-push dry run of a branch that is not merged yet.
#
# POSIX sh. Needs git, python3 and gh.
set -eu
unset CDPATH
ROOT=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
CFG=addons/GdPlanningAI/plugin.cfg
BASE=${RELEASE_BASE:-origin/main}

usage() { sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }
die() { printf 'release: %s\n' "$1" >&2; exit 1; }
say() { printf 'release: %s\n' "$1"; }
need() { command -v "$1" >/dev/null 2>&1 || die "missing required tool: $1"; }
version() {
    case "$1" in (*[!0-9.]*|""|.*|*.|*..*) usage;; esac
    case "$1" in ?*.?*.?*) ;; (*) usage;; esac
}
# The version a ref's plugin.cfg carries.
version_at() { git -C "$ROOT" show "$1:$CFG" | sed -n 's/^version="\([^"]*\)".*/\1/p'; }
tag_exists() { git -C "$ROOT" ls-remote --exit-code --tags origin "refs/tags/v$1" >/dev/null 2>&1; }

# Stop on a failed run, saying what failed and what to do: each failed
# job and step with its last error lines. No automatic retry: a real
# failure fails again, and a runner flake is for a person to call.
# $1 is the run id, $2 how to fix a real failure.
explain_failed() {
    run=$1
    printf 'release: run %s failed: %s\n' "$run" "$(gh run view "$run" --json url --jq .url)" >&2
    gh run view "$run" --json jobs \
        --jq '.jobs[] | select(.conclusion == "failure") | [.databaseId, .name, ([.steps[] | select(.conclusion == "failure") | .name] | join(", "))] | @tsv' |
        while IFS="$(printf '\t')" read -r job name steps; do
            printf '  %s, step "%s":\n' "$name" "$steps" >&2
            gh api --allow-escape-sequences "repos/{owner}/{repo}/actions/jobs/$job/logs" 2>/dev/null |
                sed -e "s/$(printf '\033')\[[0-9;]*[A-Za-z]//g" -e 's/^[0-9T:.-]*Z //' | tr -d '\033' |
                grep -E '^ *(error|Error|ERROR)[:[ ]|failed to |FAIL|panicked|##\[error\]' |
                grep -v 'Process completed with exit code' | tail -n 3 | sed 's/^/      /' >&2
        done
    printf 'release: if that is unrelated to this release (a runner flake), rerun it: gh run rerun %s --failed\n' "$run" >&2
    printf 'release: otherwise %s. Either way, then rerun: sh scripts/release.sh %s (it resumes)\n' "$2" "$VER" >&2
    exit 1
}

# Step 1: the release branch and PR. Sets PR and HEAD_SHA.
open_pr() {
    [ -z "$NOTES_FILE" ] || NOTES_FILE=$(cd -- "$(dirname -- "$NOTES_FILE")" && pwd -P)/$(basename -- "$NOTES_FILE")
    [ -z "$NOTES_FILE" ] || [ -s "$NOTES_FILE" ] || die "notes file $NOTES_FILE is missing or empty"
    BRANCH="release-$VER"

    # A detached scratch worktree: no local branch, so it works while
    # release-X.Y.Z is checked out elsewhere. Kept only by a successful
    # --no-push. LFS objects stay unfetched: a release commit touches none.
    WT=$(mktemp -d "${TMPDIR:-/tmp}/gdplanningai-$BRANCH-XXXXXX")
    KEEP=0
    cleanup() {
        [ "$KEEP" = 0 ] || return 0
        git -C "$ROOT" worktree remove --force "$WT" 2>/dev/null || rm -rf "${WT:?}"
        git -C "$ROOT" worktree prune
    }
    trap cleanup EXIT
    export GIT_LFS_SKIP_SMUDGE=1
    # Rerun: continue the pushed branch, merging main in rather than rebasing.
    if git -C "$ROOT" fetch --quiet origin "refs/heads/$BRANCH:refs/remotes/origin/$BRANCH" 2>/dev/null; then
        git -C "$ROOT" worktree add --quiet --detach "$WT" "origin/$BRANCH"
        git -C "$WT" merge --quiet --no-edit "$BASE"
    else
        git -C "$ROOT" worktree add --quiet --detach "$WT" "$BASE"
    fi
    # This script's own release-meta.py, run against the worktree.
    meta() { RELEASE_ROOT="$WT" python3 "$ROOT/scripts/release-meta.py" "$@"; }

    sed -i.bak 's/^version="[^"]*"/version="'"$VER"'"/' "$WT/$CFG"
    rm -f "${WT:?}/$CFG.bak"
    meta check "$VER" || die "$CFG not bumped to $VER"
    meta changelog "$VER" "$(date +%Y-%m-%d)" ${NOTES_FILE:+"$NOTES_FILE"} \
        || die "no CHANGELOG notes for $VER"
    meta stale \
        || die "a version is hardcoded outside plugin.cfg; read it from the build instead"

    git -C "$WT" add "$CFG" CHANGELOG.md
    git -C "$WT" diff --cached --quiet || git -C "$WT" commit --quiet -m "release $VER"
    HEAD_SHA=$(git -C "$WT" rev-parse HEAD)
    if [ "$PUSH" = 0 ]; then
        KEEP=1
        say "committed for $BRANCH in $WT (not pushed; remove with git worktree remove)"
        git -C "$WT" log --oneline "$BASE.."
        return
    fi
    git -C "$WT" push --quiet origin "HEAD:refs/heads/$BRANCH"
    PR=$(gh pr list --head "$BRANCH" --state open --json url --jq '.[0].url // empty')
    if [ -n "$PR" ]; then
        say "updated $PR"
    else
        # shellcheck disable=SC2016 # the backticks are Markdown
        BODY=$(printf 'Sets the version to %s and turns its CHANGELOG section into the release notes.\n\nOpened by `sh scripts/release.sh %s`, which merges this PR once the required check passes, tags the merge commit, and lets the tag publish the release.\n\n## Release notes\n\n%s\n' \
            "$VER" "$VER" "$(meta notes "$VER")")
        PR=$(gh pr create --base main --head "$BRANCH" --title "release $VER" --body "$BODY")
        say "opened $PR"
    fi
}

# Step 2: wait for the PR's required checks, then merge exactly that head.
merge_pr() {
    say "waiting for the required checks on $PR"
    # The required check (ci) is an aggregate GitHub creates only after the
    # jobs finish, so wait as long as any check is running, and stop at the
    # first failure. gh exits 8 while checks are pending.
    i=0; idle=0
    while :; do
        checks=$(gh pr checks "$PR" --json name,bucket,link 2>/dev/null || true)
        [ -n "$checks" ] || checks='[]'
        failed=$(printf '%s' "$checks" | python3 -c 'import json,sys; print(next((c["link"] for c in json.load(sys.stdin) if c["bucket"] in ("fail", "cancel")), ""))')
        if [ -n "$failed" ]; then
            run=$(printf '%s\n' "$failed" | sed -n 's|.*/runs/\([0-9]*\)/.*|\1|p')
            [ -n "$run" ] || die "a check failed on $PR: $failed"
            explain_failed "$run" "fix it on branch release-$VER"
        fi
        [ "$(gh pr checks "$PR" --required --json name --jq length 2>/dev/null || echo 0)" -gt 0 ] && break
        state=$(printf '%s' "$checks" | python3 -c 'import json,sys; c=json.load(sys.stdin); print("none" if not c else "pending" if any(x["bucket"] == "pending" for x in c) else "done")')
        i=$((i + 1))
        if [ "$state" = pending ]; then idle=0; else idle=$((idle + 1)); fi
        case "$state" in
            none) [ $i -le 20 ] || die "no checks started on $PR after 10 minutes" ;;
            done) [ $idle -le 10 ] || die "every check on $PR finished but no required check reported" ;;
        esac
        [ $i -le 360 ] || die "the checks on $PR are still running after 3 hours"
        sleep 30
    done
    if ! gh pr checks "$PR" --required --watch --fail-fast --interval 60 >/dev/null; then
        run=$(gh pr checks "$PR" --required --json bucket,link \
            --jq '[.[] | select(.bucket == "fail")][0].link // empty' | sed -n 's|.*/runs/\([0-9]*\)/.*|\1|p')
        [ -n "$run" ] || die "a required check failed on $PR"
        explain_failed "$run" "fix it on branch release-$VER"
    fi
    gh pr merge "$PR" --merge --match-head-commit "$HEAD_SHA"
    say "merged $PR"
}

# Step 3: tag the release PR's merge commit and push the tag.
push_tag() {
    SHA=$(gh pr list --head "release-$VER" --state merged --json mergeCommit --jq '.[0].mergeCommit.oid // empty')
    [ -n "$SHA" ] || die "origin/main is at $VER but no merged release-$VER PR was found; tag it by hand"
    git -C "$ROOT" fetch --quiet origin main
    git -C "$ROOT" merge-base --is-ancestor "$SHA" origin/main || die "merge commit $SHA is not on origin/main"
    [ "$(version_at "$SHA")" = "$VER" ] || die "merge commit $SHA is not at $VER"
    git -C "$ROOT" tag -f -a "v$VER" -m "v$VER" "$SHA"
    git -C "$ROOT" push --quiet origin "refs/tags/v$VER"
    say "pushed v$VER at $SHA"
}

# Step 4: the tag's release run builds and publishes; follow it.
watch_publish() {
    RUN=""; i=0
    while [ -z "$RUN" ]; do
        i=$((i + 1)); [ $i -le 30 ] || die "no release run for v$VER appeared; check the Actions tab"
        sleep 5
        RUN=$(gh run list --workflow release.yml --event push --limit 20 --json databaseId,headBranch \
            --jq "[.[] | select(.headBranch == \"v$VER\")][0].databaseId // empty")
    done
    say "building and publishing in run $RUN"
    gh run watch "$RUN" --exit-status --interval 60 >/dev/null \
        || explain_failed "$RUN" "fix it on main and release a new version: v$VER is tagged and tags cannot move"
    say "published $(gh release view "v$VER" --json url --jq .url)"
}

VER="${1:-}"; shift || usage; version "$VER"
NOTES_FILE=""; PUSH=1
while [ $# -gt 0 ]; do
    case "$1" in
        --notes-file) NOTES_FILE="${2:-}"; shift 2 || usage ;;
        --no-push) PUSH=0; shift ;;
        *) usage ;;
    esac
done
need git; need python3; need gh
git -C "$ROOT" fetch --quiet origin main
# Resume from wherever an earlier run stopped.
if tag_exists "$VER"; then
    gh release view "v$VER" --json isDraft --jq .isDraft 2>/dev/null | grep -qx false \
        && die "v$VER is already published"
    watch_publish
    exit 0
fi
if [ "$(version_at "$BASE")" != "$VER" ]; then
    open_pr
    [ "$PUSH" = 1 ] || exit 0
    merge_pr
fi
push_tag
watch_publish
