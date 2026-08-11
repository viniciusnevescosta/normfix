# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.3.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.0/normfix-aarch64-macos.tar.gz"
      sha256 "33fb80ca1f5d4be2f75c1bd0e5c07184da075a40c574fb3743d0f5c5d18e3c17"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.0/normfix-x86_64-macos.tar.gz"
      sha256 "d94dd116ff8da25d7d4dfd2b3eb119c96107f58ac72cb2e968613caf9df34d4c"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.0/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "ec91eaae5ce97b88aabf7bb97627ea6e84fbeb61283d64835fb910d4491780a3"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.0/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "ce03013a74dc0f4618072b7a99bb4c990737df001589befb197f23ead6621496"
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
