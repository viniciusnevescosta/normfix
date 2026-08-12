# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.5.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.5.0/normfix-aarch64-macos.tar.gz"
      sha256 "aeea8bd13d0922385cfc4f567490d7e600ecdf50431de3e55709a6939419ecb6"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.5.0/normfix-x86_64-macos.tar.gz"
      sha256 "ffc3b9a41a6d09fca2623c01170c3d4940570ef9380336c99c0c052e81581a63"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.5.0/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "24e4bec7b062406a1417d931659be55aff7c02ef8c94215b88bba105bcb302b4"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.5.0/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "c909f805801d90943dfb4f1b58c4635beca53a69dd821e4eb709cb667d733b3e"
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
