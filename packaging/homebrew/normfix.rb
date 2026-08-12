# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.3.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.1/normfix-aarch64-macos.tar.gz"
      sha256 "6419e017dcc3468e656b6f8e5a263b3e297d2d02e35248f7bd32674aeca5221b"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.1/normfix-x86_64-macos.tar.gz"
      sha256 "3351596a9984dd15c00f5d6f5891a455f9a303ba9875a473d36c15c474d0f2df"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.1/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "195e07ed2430fe9317c7b19b239ad864dbc2de91e4c17e014920e5adb24a9283"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.3.1/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "ca8a454c2bf032a5d9d01942e5908e48b0fcfb182a9f2799023ca9d3314662e8"
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
