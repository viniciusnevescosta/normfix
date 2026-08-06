# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "0.4.0-beta.5"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.5/normfix-aarch64-macos.tar.gz"
      sha256 "2211c02cd994d3215b3a9e382a8674781ac5369370dca80acddc412e9cb5e5a7"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.5/normfix-x86_64-macos.tar.gz"
      sha256 "828a3e8c2771900737de4c0202a501be02044e5eda2c7d5bb921501c87c0047d"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.5/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "07d7f447cfe4e72b8b0475c7e3cf7dc144b9f0e8cc6cca3b9469f22171b5c18e"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.5/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "417530e46b09f9e4249675a4407a04ac4fdd9e332a8f89398135da8eb3e0e6fa"
    end
  end

  def install
    bin.install "normfix"
  end

  def caveats
    <<~EOS
      normfix requires the official Norminette 3.3.59, which is a Python
      package rather than a Homebrew formula:

        pipx install norminette==3.3.59

      Any other Norminette release is rejected rather than accepted with a
      warning. Documentation: https://normfix.vercel.app/docs
    EOS
  end

  test do
    assert_match "normfix #{version}", shell_output("#{bin}/normfix --version")
    assert_match "TOO_MANY_LINES", shell_output("#{bin}/normfix explain TOO_MANY_LINES")
  end
end
