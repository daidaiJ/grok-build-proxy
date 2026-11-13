#!/usr/bin/env bash
# LOCAL: keep the upstream sync line on its own branch (`upstream`), separate
# from this fork's own commits on `main`.
#
# Upstream lands as commits titled "Synced from monorepo" (author
# grokkybara[bot], `Source-Revision: <monorepo-sha>` trailer). While that line
# stays clean, the fork's entire delta is:
#
#   git log  upstream..main
#   git diff upstream main
#
# Usage:
#   scripts-local/upstream-branch.sh                 # status (read-only)
#   scripts-local/upstream-branch.sh sync            # move `upstream` to the newest sync snapshot
#   scripts-local/upstream-branch.sh sync --force    # ... even when that drops upstream commits
#   FORK_BRANCH=x UPSTREAM_BRANCH=y scripts-local/upstream-branch.sh status
#
# `sync` covers both shapes a sync can arrive in:
#   - the snapshot commit chains onto the current upstream tip  -> move the ref
#     (original SHA kept);
#   - the snapshot landed on fork commits                       -> graft a commit
#     with the original tree/message/author onto `upstream`, so `upstream` never
#     carries fork commits.
# It is idempotent: re-running on the snapshot already held does nothing.
# Neither path touches the index, the worktree, or `main`.
set -euo pipefail

SUBJECT="Synced from monorepo"
BRANCH="${UPSTREAM_BRANCH:-upstream}"
FORK="${FORK_BRANCH:-main}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

tip() { git rev-parse --verify --quiet "refs/heads/$BRANCH" || true; }

newest_sync() { git log --format=%H --grep="^${SUBJECT}\$" "$1" 2>/dev/null | head -n1; }

short() { git log -1 --format='%h %ad %s' --date=short "$1"; }

# True when `ref`'s ancestry holds nothing but upstream snapshots and root
# commits (the root is upstream's open-source publication commit, which carries
# no sync subject).
pure() {
    local ref="$1" sha
    while IFS= read -r sha; do
        [ -n "$sha" ] || continue
        # `git rev-list --parents` prints the commit followed by its parents, so
        # a single word means a root commit.
        if [ "$(git rev-list --parents -n1 "$sha" | wc -w)" -gt 1 ]; then
            return 1
        fi
    done < <(git log --format='%H %s' "$ref" | grep -v " ${SUBJECT}\$" | cut -d' ' -f1)
    return 0
}

# Re-commit `src`'s tree on top of the current upstream tip, keeping the
# original message and identity. Prints the new commit.
graft() {
    local src="$1" tree
    tree=$(git rev-parse "${src}^{tree}")
    GIT_AUTHOR_NAME=$(git log -1 --format=%an "$src") \
        GIT_AUTHOR_EMAIL=$(git log -1 --format=%ae "$src") \
        GIT_AUTHOR_DATE=$(git log -1 --format=%aI "$src") \
        GIT_COMMITTER_NAME=$(git log -1 --format=%cn "$src") \
        GIT_COMMITTER_EMAIL=$(git log -1 --format=%ce "$src") \
        GIT_COMMITTER_DATE=$(git log -1 --format=%cI "$src") \
        git commit-tree "$tree" -p "$(tip)" <<<"$(git log -1 --format=%B "$src")"
}

cmd_status() {
    local up; up=$(tip)
    if [ -z "$up" ]; then
        printf 'no %s branch yet\n' "$BRANCH"
        return 0
    fi
    local purity='pure upstream snapshots'
    pure "$up" || purity='CONTAINS FORK COMMITS'
    printf 'upstream tip : %s\n' "$(short "$up")"
    printf 'ancestry     : %s commits (%s)\n' "$(git rev-list --count "$up")" "$purity"
    printf 'fork delta   : %s commits on %s\n' "$(git rev-list --count "$up..$FORK")" "$FORK"
    printf 'diffstat     : %s\n' "$(git diff --shortstat "$up" "$FORK" | sed 's/^ *//')"

    local newest; newest=$(newest_sync "$FORK")
    if [ -z "$newest" ]; then
        printf 'pending      : no sync commit reachable from %s\n' "$FORK"
    elif [ "$(git rev-parse "${newest}^{tree}")" != "$(git rev-parse "${up}^{tree}")" ]; then
        printf 'pending      : newer sync %s is not on %s — run: %s sync\n' \
            "$(git log -1 --format=%h "$newest")" "$BRANCH" "$0"
    fi
}

cmd_sync() {
    local force="${1:-}"
    local newest; newest=$(newest_sync "$FORK")
    [ -n "$newest" ] || die "no '${SUBJECT}' commit reachable from $FORK"

    local up; up=$(tip)
    local snap_tree; snap_tree=$(git rev-parse "${newest}^{tree}")

    if [ -n "$up" ] && [ "$(git rev-parse "${up}^{tree}")" = "$snap_tree" ]; then
        printf 'up to date: %s already holds %s (tree %s)\n' \
            "$BRANCH" "$(git log -1 --format=%h "$newest")" "${snap_tree:0:12}"
        return 0
    fi

    if pure "$newest"; then
        if [ -n "$up" ] && ! git merge-base --is-ancestor "$up" "$newest"; then
            [ "$force" = "--force" ] || die \
                "moving $BRANCH to $(git log -1 --format=%h "$newest") drops commits from $BRANCH — re-run with --force"
        fi
        git update-ref "refs/heads/$BRANCH" "$newest"
        printf 'moved %s -> %s (original SHA, upstream line was clean)\n' "$BRANCH" "$(short "$newest")"
        return 0
    fi

    [ -n "$up" ] || die "$BRANCH does not exist and $(git log -1 --format=%h "$newest") \
is not a pure upstream commit — create $BRANCH at a pure snapshot first"
    local sha; sha=$(graft "$newest")
    git update-ref "refs/heads/$BRANCH" "$sha"
    printf 'grafted %s -> %s on %s (that sync carries fork commits; its snapshot tree was re-committed)\n' \
        "$(git log -1 --format=%h "$newest")" "${sha:0:12}" "$BRANCH"
}

case "${1:-status}" in
status) cmd_status ;;
sync) shift; cmd_sync "${1:-}" ;;
-h | --help | help)
    printf 'usage: %s [status | sync [--force]]\n' "$0"
    printf '  status  show the upstream tip, its purity, and the fork delta (default)\n'
    printf '  sync    bring %s up to the newest sync snapshot reachable from %s\n' "$BRANCH" "$FORK"
    ;;
*) die "unknown command: $1 (use: status | sync)" ;;
esac
