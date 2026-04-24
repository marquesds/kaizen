# Homebrew tap: `kaizen-cli`

The [`kaizen-cli.rb`](kaizen-cli.rb) formula in this directory installs the **`kaizen`**
binary from [GitHub Releases](https://github.com/marquesds/kaizen/releases) (see
[`.github/workflows/release.yml`](../../.github/workflows/release.yml) for asset names). The
[crates.io](https://crates.io/crates/kaizen-cli) package name is `kaizen-cli`; the on-disk CLI
name stays `kaizen`.

## One-time: create a tap and add the formula

1. On GitHub, create a repository named `homebrew-tap` under your org or user (e.g.
   `https://github.com/marquesds/homebrew-tap`).
2. Add `Formula/kaizen-cli.rb` (copy from [`kaizen-cli.rb`](kaizen-cli.rb) in this folder).
3. After a **tagged** release, download each `kaizen-v<ver>-<triple>.tar.gz` from the release page
   (or use the files attached to the release in CI) and set each `sha256` in the formula:

   ```bash
   shasum -a 256 kaizen-v0.1.0-aarch64-apple-darwin.tar.gz
   ```

   or paste the value from the matching `kaizen-*.tar.gz.sha256` file in the release assets.
4. Set `version` in the formula to the released SemVer (e.g. `0.1.0`) and ensure each `url` uses the
   matching `v<version>` tag in the path.
5. Commit and push the tap. Users can install with:

   ```bash
   brew tap marquesds/tap
   brew install kaizen-cli
   ```

   (Replace `marquesds` with your GitHub user or org that owns `homebrew-tap`.)

## Uninstall

```bash
brew uninstall kaizen-cli
```

## Optional automation

A GitHub Action in the main `kaizen` repo can bump `version` and `sha256` in the tap on each
release using a personal access token with `contents: write` on the tap repository. That is
optional; the first release is usually updated manually as above.
