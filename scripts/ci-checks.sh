#!/usr/bin/env bash
set -euo pipefail

if [[ "${GITHUB_EVENT_NAME:-}" == "pull_request" ]]; then
  pull_request_number="$(jq -r '.pull_request.number' "${GITHUB_EVENT_PATH}")"

  files_modified="$(
    gh api \
      --header 'Accept: application/vnd.github+json' \
      --header 'X-GitHub-Api-Version: 2022-11-28' \
      --paginate \
      "repos/${GITHUB_REPOSITORY}/pulls/${pull_request_number}/files" \
      --jq '.[].filename'
  )"

  if ! grep -qE '^src/' <<< "${files_modified}"; then
    echo "No files in src/ modified. Skipping cargo hack."
    exit 0
  fi

  echo "::group::Changed folders under src/"
  echo "${files_modified}" \
    | grep '^src/' \
    | awk -F/ '{ print $2 }' \
    | sort -u \
    | while read -r folder; do
        echo "- src/$folder"
      done
  echo "::endgroup::"
fi

echo "Running: cargo hack check --all-targets --each-feature"
cargo hack check --all-targets --each-feature
