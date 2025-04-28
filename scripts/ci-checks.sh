#! /usr/bin/env bash
set -euo pipefail

# The below script is run on the github actions CI
# Obtain a list of workspace members
workspace_members="$(
  cargo metadata --format-version 1 --no-deps \
    | jq \
      --compact-output \
      --monochrome-output \
      --raw-output \
      '(.workspace_members | sort) as $package_ids | .packages[] | select(IN(.id; $package_ids[])) | .name'
)"

PACKAGES_CHECKED=()
PACKAGES_SKIPPED=()

# If we are running this on a pull request, then only check for packages that are modified
if [[ "${GITHUB_EVENT_NAME:-}" == 'pull_request' ]]; then
  # Obtain the pull request number and files modified in the pull request
  pull_request_number="$(jq --raw-output '.pull_request.number' "${GITHUB_EVENT_PATH}")"
  files_modified="$(
    gh api \
      --header 'Accept: application/vnd.github+json' \
      --header 'X-GitHub-Api-Version: 2022-11-28' \
      --paginate \
      "https://api.github.com/repos/${GITHUB_REPOSITORY}/pulls/${pull_request_number}/files" \
      --jq '.[].filename'
  )"

  while IFS= read -r package_name; do
    # Obtain pipe-separated list of transitive workspace dependencies for each workspace member
    change_paths="$(cargo tree --all-features --no-dedupe --prefix none --package "${package_name}" \
      | awk '{ print $1 }' | sort --unique | paste -d '|' -s -)"

    # A package must be checked if any of its transitive dependencies (or itself) has been modified
    if grep --quiet --extended-regexp "^(${change_paths})" <<< "${files_modified}"; then
      printf '::debug::Checking `%s` since at least one of these paths was modified: %s\n' "${package_name}" "${change_paths[*]//|/ }"
      PACKAGES_CHECKED+=("${package_name}")
    else
      printf '::debug::Skipping `%s` since none of these paths were modified: %s\n' "${package_name}" "${change_paths[*]//|/ }"
      PACKAGES_SKIPPED+=("${package_name}")
    fi
  done <<< "${workspace_members}"
  printf '::notice::Packages checked: %s; Packages skipped: %s\n' "${PACKAGES_CHECKED[*]}" "${PACKAGES_SKIPPED[*]}"

  packages_checked="$(jq --compact-output --null-input '$ARGS.positional' --args -- "${PACKAGES_CHECKED[@]}")"

  crates_with_features="$(cargo metadata --format-version 1 --no-deps \
    | jq \
      --compact-output \
      --monochrome-output \
      --raw-output \
      --argjson packages_checked "${packages_checked}" \
      '[ ( .workspace_members | sort ) as $package_ids | .packages[] | select( IN( .name; $packages_checked[] ) ) | { name: .name, features: ( .features | keys ) } ]')"
else
  # If we are doing this locally or on merge queue, then check for all the crates
  crates_with_features="$(cargo metadata --format-version 1 --no-deps \
    | jq \
      --compact-output \
      --monochrome-output \
      --raw-output \
      '[ ( .workspace_members | sort ) as $package_ids | .packages[] | select( IN( .id; $package_ids[] ) ) | { name: .name, features: ( .features | keys ) } ]')"
fi

# List of cargo commands that will be executed
all_commands=()

# we will run the usual cargo hack command
crates="$(jq -r '.[] | .name' <<< "${crates_with_features}")"

while IFS= read -r crate && [[ -n "${crate}" ]]; do
  command="cargo hack check --all-targets --each-feature --package \"${crate}\""
  all_commands+=("$command")
done <<< "${crates}"

if ((${#all_commands[@]} == 0)); then
  echo "There are no commands to be executed"
  exit 0
fi

echo "The list of commands that will be executed:"
printf "%s\n" "${all_commands[@]}"
echo

# Execute the commands
for command in "${all_commands[@]}"; do
  if [[ "${CI:-false}" == "true" && "${GITHUB_ACTIONS:-false}" == "true" ]]; then
    printf '::group::Running `%s`\n' "${command}"
  fi

  bash -c -x "${command}"

  if [[ "${CI:-false}" == "true" && "${GITHUB_ACTIONS:-false}" == "true" ]]; then
    echo '::endgroup::'
  fi
done
