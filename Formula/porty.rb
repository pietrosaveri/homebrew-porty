class Porty < Formula
  desc "Fast, intelligent local port inspector for macOS"
  homepage "https://github.com/pietrosaveri/Porty"
  url "https://github.com/pietrosaveri/Porty/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "5559ca3f3627b324e43c16bd0f349a18cff488811b612f8e6da1146b4d4d950e"
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
