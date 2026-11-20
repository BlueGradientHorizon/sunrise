#!/usr/bin/env bash
# Unshallow this checkout's HEAD for the gate's history checks (public-tree ranges, sync ancestry, selection): two
# tries of at most 300 s each, then fail closed naming $FETCH_STEP.
#
# On a Dyna runner a bare mirror of the repository is kept in $DYNA_CI_KEEP, the host's folder for this repository
# that every slot shares (a slot's own $DYNA_CI_BUILD is emptied whenever the slot switches repositories, so a mirror
# there was cold on about half the gate's jobs). The checkout borrows its objects as an alternate, so GitHub sends
# only what the last job on this host didn't see (a full unshallow was about 146 s), and the mirror takes this head
# afterwards. One job at a time updates it (a lock; a job that finds it held skips the update), and a job never
# deletes it, since another job's checkout may be borrowing from it. Where $DYNA_CI_KEEP is not given (a fork's pull
# request), the slot's folder holds a mirror of its own as before, and a broken one is dropped. A mirror that is
# missing or broken costs nothing but the saving: the fetch goes on without it.
#
# HISTORY_FILTER (a `git fetch --filter` spec) fetches less than the whole history, for a step that reads only part of
# it: tree:0 brings the commits alone, and Git fetches a tree from GitHub when a command first reads it. Such a
# checkout neither borrows from the slot's mirror nor fills it, since the mirror must hold every object.
set -uo pipefail
step=${FETCH_STEP:?FETCH_STEP names the step in errors}
# Seconds from this script's start that the mirror's upkeep may use: the steps' timeout-minutes is 11.
UPKEEP_BUDGET=${UPKEEP_BUDGET:-600}
[ "$(git rev-parse --is-shallow-repository)" = false ] && exit 0
head=$(git rev-parse HEAD)
alternates=$(git rev-parse --git-path objects/info/alternates)
filter=${HISTORY_FILTER:-}
mirror= borrowed= shared=
if [ -n "${DYNA_CI_KEEP:-}" ] && [ -z "$filter" ]; then
  mirror="$DYNA_CI_KEEP/git-mirror/${GITHUB_REPOSITORY:?}.git" shared=1
elif [ -n "${DYNA_CI_BUILD:-}" ] && [ -z "$filter" ]; then
  mirror="$DYNA_CI_BUILD/git-mirror/${GITHUB_REPOSITORY:?}.git"
fi
if [ -n "$mirror" ]; then
  if git -C "$mirror" rev-parse -q --verify refs/heads/ci-head >/dev/null 2>&1; then
    echo "$mirror/objects" >> "$alternates"
    borrowed=1
  fi
fi
for attempt in 1 2; do
  if timeout --kill-after=10s 300s git fetch -q --no-tags --unshallow ${filter:+"--filter=$filter"} origin "$head"; then
    if [ -n "$mirror" ]; then
      # Keep this head for the next job; a failure only loses the saving. A mirror this checkout borrows from stays
      # for the job's later steps, and the next job, finding it unusable, borrows nothing and drops it here. The
      # upkeep gets only what is left of the step's 11 minutes, less a margin, so it never costs the job.
      left=$((UPKEEP_BUDGET - SECONDS))
      mkdir -p "$(dirname "$mirror")"
      exec 9> "$mirror.lock"
      if [ "$left" -lt 15 ]; then
        echo "$step: no time left to update the history mirror."
      elif ! flock -n 9; then
        echo "$step: another job is updating the history mirror; this one leaves it to that job."
      else
        git init -q --bare "$mirror" &&
          timeout --kill-after=5s "${left}s" git -C "$mirror" fetch -q --no-tags "$(git rev-parse --show-toplevel)" \
            "+$head:refs/heads/ci-head"
        status=$?
        if [ "$status" -eq 0 ]; then
          # A repack cut short leaves the mirror as it was: packs and refs are written whole, then renamed.
          left=$((UPKEEP_BUDGET - SECONDS))
          [ "$left" -lt 15 ] || timeout --kill-after=5s "${left}s" git -C "$mirror" -c gc.autoDetach=false gc --auto \
            --quiet || echo "$step: the history mirror was not repacked this time."
        elif [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
          echo "$step: out of time to update the history mirror; the next job takes it from where it is."
        else
          echo "$step: the history mirror could not take this head."
          [ -n "$borrowed$shared" ] || rm -rf "$mirror"
        fi
      fi
      exec 9>&-
    fi
    exit 0
  fi
  echo "$step: GitHub fetch attempt $attempt failed or timed out."
  if [ -n "$borrowed" ]; then
    # The objects fetched before the alternate was added stand alone. Retry without the mirror, in case it is what
    # broke; a mirror that then can't take the head is dropped above.
    echo "$step: retrying without the slot's history mirror."
    sed -i "\\|^$mirror/objects\$|d" "$alternates"
    borrowed=
  fi
done
echo "::error::$step: history fetch from GitHub failed or timed out twice."
exit 1
