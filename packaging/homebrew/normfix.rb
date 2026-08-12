# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.3.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.2/normfix-aarch64-macos.tar.gz"
      sha256 "de8469efe0dd261b8d04fbac678627e44226b1005d3dfcc4caf50607bc8732bb"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.2/normfix-x86_64-macos.tar.gz"
      sha256 "51ddec7c0621ce89565c663ebff72df834cdf51e1a68186d62b0eeb17d2cf7ed"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.2/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "0df22cec4fcb72e2bd5a5b90a7c3d0661e6d0e58df46f052c8158f57d1e23546"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.2/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "1b0607ca56d3a50e982b5cb3ac1dea9b7df57a2964f5ccf73c0c641674682f3b"
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
