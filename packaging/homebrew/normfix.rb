# Homebrew formula for normfix.
#
# Source of truth for the tap at viniciusnevescosta/homebrew-normfix. Update it
# together with a release: the version, the four URLs, and the four checksums
# all come from the published SHA256SUMS manifest.
class Normfix < Formula
  desc "Safe automatic fixes and clear diagnostics for the 42 Norm"
  homepage "https://normfix.vercel.app"
  version "1.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.1.0/normfix-aarch64-macos.tar.gz"
      sha256 "9a6a0f965569d1967883fa7acf94bdcce6234efd7541904034309b6ebd0f752a"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.1.0/normfix-x86_64-macos.tar.gz"
      sha256 "8192037884c2e94f8deb5e6f696221d223bb31d68af63bb8ac4373926a12b373"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.1.0/normfix-aarch64-linux-gnu.tar.gz"
      sha256 "ebfc270ad49d21e5f0ab2cc170b8a981d74fc115a181ad0dc614270d72230725"
    end
    on_intel do
      url "https://github.com/viniciusnevescosta/normfix/releases/download/v1.1.0/normfix-x86_64-linux-gnu.tar.gz"
      sha256 "87cd30d891fb8db766ddd0b510c766cd8615be47849d3de707ed426e9a50b3c4"
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
