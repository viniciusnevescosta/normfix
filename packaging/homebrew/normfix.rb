# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.0.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0/normfix-aarch64-macos.tar.gz"
      sha256 "c4e4da19554d1c5dffa7513504d4a68bb1071f3e70c871db11f8dbb0b429f5a7"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0/normfix-x86_64-macos.tar.gz"
      sha256 "a611ca0e99b50957b7c744ac90e3493aa61439c07ded77ff886dc573f0d94733"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "5f570bb38dbe4dfcafb4289c070d6e2ab0320bfe66deb15668c619e665625f50"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "c87be18c060fa348fec57522ff82a8b7254cdf087d51386c856d265988b16d8a"
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
