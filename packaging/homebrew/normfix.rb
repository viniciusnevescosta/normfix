# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.0.0-rc.3"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.3/normfix-aarch64-macos.tar.gz"
      sha256 "c8f93def5b35d56401e14d0fe95569b64d334f91be044991acad0aaa4a516cb2"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.3/normfix-x86_64-macos.tar.gz"
      sha256 "10cb8eb278c08ef310ae2436008d21bf217060cf144b24e6d30e46a63ac15c7d"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.3/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "2806d21bce798d2ec2c725084245576a04e63c095457bbef64156f24e14a86d7"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.0.0-rc.3/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "c1ef7a05c2a3408d719fb0699ec7720c2665f22ae353b9a9e209b73f192b5d84"
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
