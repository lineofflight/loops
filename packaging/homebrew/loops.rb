# Rendered from packaging/homebrew/loops.rb in lineofflight/loops on each release.
class Loops < Formula
  desc "Open loops ledger for coding agents"
  homepage "https://lineofflight.dev/loops/"
  version "@VERSION@"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/lineofflight/loops/releases/download/@TAG@/loops-@TAG@-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA256_AARCH64_APPLE_DARWIN@"
    end
    on_intel do
      url "https://github.com/lineofflight/loops/releases/download/@TAG@/loops-@TAG@-x86_64-apple-darwin.tar.gz"
      sha256 "@SHA256_X86_64_APPLE_DARWIN@"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/lineofflight/loops/releases/download/@TAG@/loops-@TAG@-aarch64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256_AARCH64_UNKNOWN_LINUX_MUSL@"
    end
    on_intel do
      url "https://github.com/lineofflight/loops/releases/download/@TAG@/loops-@TAG@-x86_64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256_X86_64_UNKNOWN_LINUX_MUSL@"
    end
  end

  def install
    bin.install "loops"
  end

  def caveats
    <<~EOS
      Add loops to Claude Code:
        claude mcp add loops -- loops mcp

      Other agents take the same command: loops mcp
    EOS
  end

  test do
    assert_match "1. Check the formula", shell_output("#{bin}/loops add 'Check the formula'")
  end
end
