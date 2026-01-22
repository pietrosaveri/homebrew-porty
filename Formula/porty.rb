class Porty < Formula
  desc "Fast, intelligent local port inspector for macOS"
  homepage "https://github.com/pietrosaveri/Porty"
  url "https://github.com/pietrosaveri/homebrew-porty/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "43096d6c37ec31ab0a505e2e47abd4c3a9ca12ee32511776296ae8f3309ceb44"
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

