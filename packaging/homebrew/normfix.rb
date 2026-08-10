# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.0.0-rc.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.2/normfix-aarch64-macos.tar.gz"
      sha256 "9d937c235e7d9363ad9311ff847c5c499d632f54a0dfb74d028a57b8ff553596"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.2/normfix-x86_64-macos.tar.gz"
      sha256 "c471f0030c59862659d0cf8ad93b08d75927694a085341407b6776ce9983f4ce"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.2/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "90097bec5a1436815e9d7e8d20144c6c80d1bcf19b3e67eba349eb543495771e"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.2/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "e91c7e13811c38b65537174d1e2b675af7c3e342f490fcb342af8d619dd0779d"
    end
  end

  def install
    bin.install "normfix"
  end

  def caveats
    <<~EOS
      normfix uses the official Norminette, which is a Python package rather
      than a Homebrew formula. The tested compatibility baseline is 3.3.59:

        pipx install norminette==3.3.59

      Another parseable release continues with a prominent compatibility
      advisory. Use --strict-norminette-version in pinned CI.
      Documentation: https://normfix.vercel.app/docs
    EOS
  end

  test do
    assert_match "normfix #{version}", shell_output("#{bin}/normfix --version")
    assert_match "TOO_MANY_LINES", shell_output("#{bin}/normfix explain TOO_MANY_LINES")
  end
end
