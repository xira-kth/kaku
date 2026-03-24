# Releasing `kaku`

## Preflight

- Ensure `main` is the default branch.
- Ensure the version in the workspace root `Cargo.toml` is correct.
- Ensure GitHub Actions is green on `main`.
- Ensure the npm scope in the root `Cargo.toml` matches a real npm org or user.

## npm Publishing

- Recommended: use npm trusted publishing with GitHub Actions OIDC.
- Do not store a long-lived npm publish token in the repository or in a checked-in `.env` file.
- Configure a trusted publisher on npm for:
  - Organization or user: `voidique`
  - Repository: `kaku`
  - Workflow filename: `release.yml`
- The release workflow already has `id-token: write`, which npm trusted publishing requires.
- After trusted publishing works, revoke any old npm publish token.

## If You Need a Temporary Fallback

- Use a GitHub Actions secret, not a committed file.
- Never put the token in:
  - `.env` committed to git
  - `.npmrc` committed to git
  - workflow YAML
  - package metadata
- If you temporarily use a token, store it only as a repository secret such as `NPM_TOKEN`, then remove it after moving to trusted publishing.

## Release Flow

1. Update the workspace version in the root `Cargo.toml`.
2. Commit the version bump to `main`.
3. Tag the release:

```bash
git tag v0.1.1
git push origin main
git push origin v0.1.1
```

4. Let the release workflow build artifacts for each target.
5. Publish through the release workflow.
6. If npm trusted publishing is configured, npm publish will use OIDC and no token is needed.

## Homebrew

- Homebrew publish is intentionally not part of the current release workflow.
- Add it back later after the tap repository and token are ready.

## Notes

- `kaku` currently ships without syntax highlighting in default builds.
- This keeps the default release smaller and avoids known unmaintained transitive dependencies pulled in by `syntect`.
- If syntax highlighting is needed for a source build, enable it explicitly with:

```bash
cargo build --features syntax
```
