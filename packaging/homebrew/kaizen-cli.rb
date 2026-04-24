# frozen_string_literal: true
#
# Copy this file to a Homebrew tap repository as Formula/kaizen-cli.rb, then replace
# the sha256 placeholders with values from the matching GitHub Release assets, or
# from the `.sha256` sidecar files next to each .tar.gz.
#
# Source assets are produced by: .github/workflows/release.yml
#   kaizen-v<VER>-<RUST_TRIPLE>.tar.gz  →  contains  kaizen-v<VER>-<RUST_TRIPLE>/kaizen

class KaizenCli < Formula
  desc "Distributable agent observability: sessions, retros, and repo-level improvement for coding agents"
  homepage "https://github.com/marquesds/kaizen"
  version "0.1.0"
  license "AGPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/marquesds/kaizen/releases/download/v#{version}/kaizen-v#{version}-aarch64-apple-darwin.tar.gz"
      # Replace with: shasum -a 256 <file>  or from release checksum file
      # Replace (64 hex chars) from release or: shasum -a 256 <file>
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
    on_intel do
      url "https://github.com/marquesds/kaizen/releases/download/v#{version}/kaizen-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/marquesds/kaizen/releases/download/v#{version}/kaizen-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
    on_intel do
      url "https://github.com/marquesds/kaizen/releases/download/v#{version}/kaizen-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    # Single top-level directory in the archive: kaizen-v<ver>-<triple>/
    bin.install Dir.glob("kaizen-*/kaizen").first
  end

  test do
    system bin/"kaizen", "--version"
  end
end
