# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "0.4.0-beta.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.1/normfix-aarch64-macos.tar.gz"
      sha256 "5a47197e051f551bc59450200a6bf862f9c2bd5d50b4319618d22c798059b380"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.1/normfix-x86_64-macos.tar.gz"
      sha256 "fdc97b5c540e2736da4e59a11d5ea444cbf13c9910ffd08937a2d19cf4b99249"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.1/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "00ea329f8ad3dfc0e56a47d7da72ec0391c5a69694307a89118049719dfa6504"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.1/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "1d9d9e04ba86a571d10f8a3768021852eafaafc3780a4be6278fe8ddd331f891"
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
