class Porty < Formula
  desc "Fast, intelligent local port inspector for macOS"
  homepage "https://github.com/pietrosaveri/Porty"
  url "https://github.com/pietrosaveri/homebrew-porty/archive/refs/tags/0.1.1.tar.gz"
  sha256 "fd022b5a239bd4cdc33219475e84a488240c3ead85374c38681585f0cb88f296"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Test that the binary runs and shows help
    assert_match "A fast, intelligent local port inspector", shell_output("#{bin}/porty --help")
  end
end

