# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.4.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.4.0/normfix-aarch64-macos.tar.gz"
      sha256 "1ac872efdb2565ac0334827cc2fe856d0f34e3b5dc10459d313024229b76848e"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.4.0/normfix-x86_64-macos.tar.gz"
      sha256 "552175816a023bdede8174a311d896edd82345cb22b5920d8cb2fac15ef5f946"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.4.0/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "742e99f155b5b2307653ee27eb76d3f7372e5aea5dbdea0a10bc5979c44e4633"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.4.0/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "6d6b4265495e084cc7925caccee5d3f3e949b3e59ba5a51d5bd6f64bff613547"
    end
  end

  def install
    bin.install "normfix"
  end

  def caveats
    <<~EOS
      normfix uses the official Norminette, which is not a Homebrew formula.
      Install it by following the instructions in its own repository, which is
      the only source that stays correct when they change:

        https://github.com/42School/norminette

      The tested compatibility baseline is 3.3.59. Another parseable release
      continues with a prominent compatibility advisory; use
      --strict-norminette-version in pinned CI.
      Documentation: https://normfix.vercel.app/docs
    EOS
  end

  test do
    assert_match "normfix #{version}", shell_output("#{bin}/normfix --version")
    assert_match "TOO_MANY_LINES", shell_output("#{bin}/normfix explain TOO_MANY_LINES")
  end
end
