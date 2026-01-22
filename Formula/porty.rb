class Porty < Formula
  desc "Fast, intelligent local port inspector for macOS"
  homepage "https://github.com/pietrosaveri/Porty"
  url "https://github.com/pietrosaveri/homebrew-porty/archive/refs/tags/v0.1.3.tar.gz"
  sha256 "7e3d438a54175f282d0e061d00e118ab9980e9183e011536663dba1b3f0d11a3"
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

