# Homebrew formula for terminal-poker
# To use: brew tap terminal-poker/tap && brew install terminal-poker

class TerminalPoker < Formula
  desc "Heads-up No-Limit Texas Hold'em training tool for the terminal"
  homepage "https://github.com/ashxudev/terminal-poker"
  version "1.0.0"
  license "MIT"

  # TODO: Update these URLs when releasing
  on_macos do
    on_arm do
      url "https://github.com/ashxudev/terminal-poker/releases/download/v1.0.0/terminal-poker-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_ARM64"
    end
    on_intel do
      url "https://github.com/ashxudev/terminal-poker/releases/download/v1.0.0/terminal-poker-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X64"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ashxudev/terminal-poker/releases/download/v1.0.0/terminal-poker-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_ARM64"
    end
    on_intel do
      url "https://github.com/ashxudev/terminal-poker/releases/download/v1.0.0/terminal-poker-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_X64"
    end
  end

  def install
    bin.install "terminal-poker"
    bin.install "poker"
  end

  test do
    assert_match "terminal-poker", shell_output("#{bin}/terminal-poker --version")
    assert_match "terminal-poker", shell_output("#{bin}/poker --version")
  end
end
