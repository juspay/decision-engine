# Nightly Build Workflow

## Overview

This workflow automatically creates nightly builds of the Decision Engine at **5:00 AM IST** every day.

## What It Does

1. **Creates a Git Tag** with format `nightly-YYYY.MM.DD` (e.g., `nightly-2026.07.21`)
2. **Builds Docker Images** for all platforms:
   - PostgreSQL variant (`ghcr.io/juspay/decision-engine/postgres`)
   - MySQL variant (`ghcr.io/juspay/decision-engine`)
   - Groovy runner (`ghcr.io/juspay/decision-engine/groovy-runner`)
3. **Creates a GitHub Release** with the nightly tag (marked as pre-release)
4. **Tags images** with both the date tag and `nightly-latest`

## Schedule

- **Automatic Run**: Daily at 5:00 AM IST (11:30 PM UTC previous day)
- **Manual Trigger**: Available via GitHub Actions UI with optional tag suffix

## Docker Images

All images are multi-architecture (amd64 + arm64) and available at:

```bash
# Specific nightly version
docker pull ghcr.io/juspay/decision-engine:nightly-2026.07.21
docker pull ghcr.io/juspay/decision-engine/postgres:nightly-2026.07.21
docker pull ghcr.io/juspay/decision-engine/groovy-runner:nightly-2026.07.21

# Latest nightly (always points to the most recent nightly build)
docker pull ghcr.io/juspay/decision-engine:nightly-latest
docker pull ghcr.io/juspay/decision-engine/postgres:nightly-latest
docker pull ghcr.io/juspay/decision-engine/groovy-runner:nightly-latest
```

### Flat alias tags (for single-repository registries)

The sub-images are additionally tagged on the **root** image with the sub-image
name folded into the tag:

```bash
docker pull ghcr.io/juspay/decision-engine:postgres-nightly-2026.07.21
docker pull ghcr.io/juspay/decision-engine:groovy-runner-nightly-2026.07.21
```

These exist for mirroring nightlies into registries that keep everything in a
single repository (e.g. a central ECR repo). The source and destination
`repo:tag` are then identical, so an image-copy job needs no name rewriting:

```
ghcr.io/juspay/decision-engine:postgres-nightly-2026.07.21  <registry>/decision-engine:postgres-nightly-2026.07.21
```

Note: a tag can never contain `:` — a destination like
`decision-engine:postgres:nightly-2026.07.21` is an invalid Docker reference.

## Manual Trigger

You can manually trigger a nightly build from the GitHub Actions UI:

1. Go to Actions → Nightly Build and Release
2. Click "Run workflow"
3. Select the `main` branch (nightlies should only be published from the default branch)
4. Optionally provide a tag suffix (e.g., `rc1` creates `nightly-2026.07.21-rc1`)

## Tag Format

- **Standard**: `nightly-YYYY.MM.DD` (e.g., `nightly-2026.07.21`)
- **With Suffix**: `nightly-YYYY.MM.DD-<suffix>` (e.g., `nightly-2026.07.21-rc1`)

## GitHub Releases

Each nightly build creates a GitHub pre-release with:
- Tag name and commit SHA
- Build timestamp
- Links to all Docker images
- Release description with build metadata and usage examples

## Configuration

The schedule is configured using GitHub Actions cron syntax:

```yaml
schedule:
  - cron: '30 23 * * *'  # 11:30 PM UTC = 5:00 AM IST
```

To change the schedule, modify the cron expression in `.github/workflows/nightly-build.yml`.

## Caching

The workflow uses GitHub Actions cache to speed up builds:
- Postgres builds: `nightly-postgres-{arch}`
- MySQL builds: `nightly-mysql-{arch}`
- Groovy builds: `nightly-groovy`

## Notes

- Nightly builds are marked as **pre-release** in GitHub
- Images are automatically pushed to GitHub Container Registry (ghcr.io)
- The workflow requires `contents: write` and `packages: write` permissions
- Build time varies but typically completes within 30-45 minutes

## Monitoring

Check the workflow status at:
https://github.com/juspay/decision-engine/actions/workflows/nightly-build.yml
