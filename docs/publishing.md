# Publishing orbbec-v1 through GitHub Actions

Repository: https://github.com/gmmyung/orbbec-v1 (public).
The public API crate is **orbbec-v1**. Every package uses that prefix:

1. `orbbec-v1-bin-x86_64`
2. `orbbec-v1-bin-aarch64`
3. `orbbec-v1-sys`
4. `orbbec-v1`
5. `orbbec-v1-viewer`

Consumers add `orbbec-v1 = "0.1.0"`; Cargo selects the supporting packages.
The unpublished maintainer tool is named `orbbec-v1-bindgen`.

## Manual workflow

Actions → **Publish orbbec-v1** → **Run workflow** on the default branch.

- `version`: must equal the API crate's current Cargo version; it does not bump versions.
- `publish=false` (default): tests, lint, isolated package verification, ARM64
  viewer linking, and a registry-aware release plan. No credentials needed.
- `publish=true`: the same verification followed by sequential publication.
- `authentication=token`: initial publication, using the Actions secret.
- `authentication=trusted`: subsequent releases, using GitHub OIDC.

Equivalent verification command:

```bash
gh workflow run publish.yml --repo gmmyung/orbbec-v1 --ref main \
  -f version=0.1.0 -f publish=false -f authentication=trusted
```

The publisher job uses the `crates-io` GitHub environment. Runs are serialized;
verification and publishing check out the same commit. No push, pull request,
or tag automatically uploads packages. Workflow actions are pinned to commits.
Artifacts contain the crate files, isolated test report, and release plan.
Verification also checks repository files and reachable Git history for crates.io
token patterns, reporting locations without printing matching values. No token
value belongs in this repository or in workflow dispatch inputs.

## First publication: replacement token

Revoke the token shared in chat and create a replacement on
https://crates.io/settings/tokens. Do not commit it, paste it in workflow inputs,
or put it on a command line. Its permissions must allow creating these new crates
and publishing versions. The workflow reads it from `secrets.CARGO_REGISTRY_TOKEN`
and passes it to the publishing step as an environment variable. A repository
Actions secret works; no duplicate `crates-io` environment secret is required.
An environment secret with the same name can also be used.

Set the secret interactively in your terminal (the value is hidden):

```bash
gh secret set CARGO_REGISTRY_TOKEN --repo gmmyung/orbbec-v1
```

After a successful verification run, trigger initial publication:

```bash
gh workflow run publish.yml --repo gmmyung/orbbec-v1 --ref main \
  -f version=0.1.0 -f publish=true -f authentication=token
```

This uploads all five packages, preserving native library licenses. The workflow
does not resolve the vendor-source/version discrepancy described in
[distribution.md](distribution.md); packaging success is not a declaration of
full LGPL compliance.

## Later releases: trusted publishing

Current crates.io documentation requires an API token for the initial publication.
Once each crate exists, configure its Settings → Trusted Publishing:

| Field | Value |
| --- | --- |
| Repository owner | `gmmyung` |
| Repository name | `orbbec-v1` |
| Workflow filename | `publish.yml` |
| Environment | `crates-io` |

Configure all five crates for the same workflow/environment. The official
`rust-lang/crates-io-auth-action` exchanges GitHub OIDC for a temporary token and
revokes it at job completion. Then delete the bootstrap repository secret:

```bash
gh secret delete CARGO_REGISTRY_TOKEN --repo gmmyung/orbbec-v1
```

Use `authentication=trusted` for later publications. Bump the appropriate Cargo
versions and dependency requirements, update Cargo.lock, commit, and push before
running the workflow. It checks the expected API version and clean package commit.
An already published version is skipped only when its packaged files match,
excluding generated Cargo.lock and VCS metadata. Different existing contents abort
the release before any uploads. Cargo waits for each new dependency to appear in
the registry index. A partially completed release can be rerun on the same commit.

References: [crates.io trusted publishing](https://crates.io/docs/trusted-publishing),
[official authentication action](https://github.com/rust-lang/crates-io-auth-action),
[Cargo publish behavior](https://doc.rust-lang.org/cargo/commands/cargo-publish.html).
