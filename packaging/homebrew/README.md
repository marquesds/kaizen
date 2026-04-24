# Homebrew tap: `kaizen-cli`

The [`kaizen-cli.rb`](kaizen-cli.rb) formula in this directory installs the **`kaizen`**
binary from [GitHub Releases](https://github.com/marquesds/kaizen/releases) (see
[`.github/workflows/release.yml`](../../.github/workflows/release.yml) for asset names). The
[crates.io](https://crates.io/crates/kaizen-cli) package name is `kaizen-cli`; the on-disk CLI
name stays `kaizen`.

## Tap repository

The tap lives at **[github.com/marquesds/homebrew-tap](https://github.com/marquesds/homebrew-tap)**.
Copy updates from [`kaizen-cli.rb`](kaizen-cli.rb) here into that repo’s `Formula/kaizen-cli.rb` when
the formula changes.

## When you cut a release (maintainers)

1. After a **tagged** release, download each `kaizen-v<ver>-<triple>.tar.gz` from the release page
   (or use the files attached to the release in CI) and set each `sha256` in the formula:

   ```bash
   shasum -a 256 kaizen-v0.1.0-aarch64-apple-darwin.tar.gz
   ```

   or paste the value from the matching `kaizen-*.tar.gz.sha256` file in the release assets.
2. Set `version` in the formula to the released SemVer (e.g. `0.1.0`) and ensure each `url` uses the
   matching `v<version>` tag in the path.
3. Commit and push **homebrew-tap**. Users install with:

   ```bash
   brew tap marquesds/tap
   brew install kaizen-cli
   ```

## Uninstall

```bash
brew uninstall kaizen-cli
```

## Automation (default)

On stable releases, [`.github/workflows/release.yml`](../../.github/workflows/release.yml) runs the
**`update-homebrew-tap`** job: it downloads the same build artifacts, runs
[`scripts/render-homebrew-tap-formula.sh`](../../scripts/render-homebrew-tap-formula.sh), and pushes
to `homebrew-tap` when the repository secret **`HOMEBREW_TAP_TOKEN`** is set (see
[`CONTRIBUTING.md`](../../CONTRIBUTING.md)). No manual sha edit is required for normal releases.

You can still run the script locally against a `dist/` directory of `.sha256` files if you need to
fix the tap outside CI.
