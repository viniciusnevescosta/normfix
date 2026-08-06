# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "0.4.0-beta.4"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.4/normfix-aarch64-macos.tar.gz"
      sha256 "74dde753caa99df7f6d754d0dabb3e1a140929a0cb07d867dc06b8770f4c78c7"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.4/normfix-x86_64-macos.tar.gz"
      sha256 "788ead69948a8deb8bfa451b618dcc6e441e1f4261e4cc4faf48bf522f464214"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.4/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "9273cd555ddffe7b8fb6efaa4c418556bd0ec8c671029b835aa6929a0d6c0131"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.4/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "12722cb000e2e24a7707b3086898f6f4847e6d6ea2cdbfc8238d932787fa5cb8"
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
