# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "0.4.0-beta.3"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.3/normfix-aarch64-macos.tar.gz"
      sha256 "a987d83f92ec663fc88c6bc5cf97773a3eb415237c71b239d20add2895994674"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.3/normfix-x86_64-macos.tar.gz"
      sha256 "4b8f6a8d46f2b8e1eab6e755fb3aca670671b49bf7efa1bbca62a67303ee67f4"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.3/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "4cd059d941919485fe1d689ba373a2f10f4c5542d39dc940073d600957fae704"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.3/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "f96907cd47e04559663bd691d7cd10f09ab3ffbbe191413ea5a92ebcaab77101"
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
