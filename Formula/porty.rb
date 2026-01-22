class Porty < Formula
  desc "Fast, intelligent local port inspector for macOS"
  homepage "https://github.com/pietrosaveri/Porty"
  url "https://github.com/pietrosaveri/homebrew-porty/archive/refs/tags/0.1.1.tar.gz"
  sha256 "d5558cd419c8d46bdc958064cb97f963d1ea793866414c025906ec15033512ed"
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

