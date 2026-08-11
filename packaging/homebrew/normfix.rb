# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.2.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.2.0/normfix-aarch64-macos.tar.gz"
      sha256 "00b8653b9bd90a67b48bfc090e9f048a7785fc08b01cd74365865f25039361ef"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.2.0/normfix-x86_64-macos.tar.gz"
      sha256 "0a899f1a9a8002c9d8a2b7b0a52f6b2938798ff64a5ccaabf8d8b3f4776848bb"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.2.0/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "03c2b1e89bde3a10168e1a8765640f7a669cf705853551638c47d8af7300ecca"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.2.0/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "fa3abdb01df50d3ef5bb06f8be426738ff8691e157763c1ef173b928a483a7af"
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
