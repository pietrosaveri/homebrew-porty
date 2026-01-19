class Porty < Formula
  desc "Fast, intelligent local port inspector for macOS"
  homepage "https://github.com/pietrosaveri/Porty"
  url "https://github.com/pietrosaveri/Porty/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5" # Will be filled after creating the release
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
