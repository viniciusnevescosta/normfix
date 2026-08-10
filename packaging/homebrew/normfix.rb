# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.0.0-rc.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.1/normfix-aarch64-macos.tar.gz"
      sha256 "cd42e0d656ae244e359a3a779c404f4cfceabf08c3b97edc8a8f0eba37ff1969"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.1/normfix-x86_64-macos.tar.gz"
      sha256 "574d421239a5148d1510d448ea68dcb7c5043efe8ad624895aa8d9a454bf5e77"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.1/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "d25fb796addb5738417e35a81455b98d10dff77d04a54c267ce6cb49b925b7ed"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.1/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "6ab6652252ba6ad7c0f8e228983f9139ec072bededa40d425f108ead48917d99"
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
