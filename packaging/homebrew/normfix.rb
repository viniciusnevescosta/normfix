# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "0.4.0-beta.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.2/normfix-aarch64-macos.tar.gz"
      sha256 "e022116af619a76fd76b94e3cb83c9614221f6449e4c1f451ef68fc00fd95c76"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.2/normfix-x86_64-macos.tar.gz"
      sha256 "8111cf9078df01ade33319021b3e1158fc8682aa775b4d40bfe1f53e16a34fd3"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.2/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "f020c0ce869eb1c6b1ddf52cfc7613e31873d3f9eee949721ca99068cfa78e76"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v0.4.0-beta.2/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "9024d4935fd416c1ac548de3b235af956d5393e01bc63791546fd8a53e886875"
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
