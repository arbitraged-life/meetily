cask "meetily" do
  version "0.3.0"
  sha256 "fcd30cbb2e78f9b4b2d1f272802c43551f70b214179edf11e0c67a6fa1db6334"

  url "https://github.com/arbitraged-life/meetily/releases/download/v#{version}/meetily_#{version}_aarch64.dmg",
      verified: "github.com/arbitraged-life/meetily/"
  name "Meetily"
  desc "Local-first meeting recorder, transcriber, and summarizer"
  homepage "https://github.com/arbitraged-life/meetily"

  # Only Apple Silicon builds are published.
  depends_on arch: :arm64
  # Built with MACOSX_DEPLOYMENT_TARGET=14.2, requires Sonoma 14.2+
  depends_on macos: ">= :sonoma_14_2"
  app "Meetily.app"
  zap trash: [
    "~/Library/Application Support/com.meetily.ai",
    "~/Library/Application Support/meetily",
    "~/Library/Caches/com.meetily.ai",
    "~/Library/HTTPStorages/com.meetily.ai",
    "~/Library/Preferences/com.meetily.ai.plist",
    "~/Library/Saved Application State/com.meetily.ai.savedState",
    "~/Library/WebKit/com.meetily.ai",
  ]

  caveats <<~EOS
    Meetily is an Apple Silicon-only build signed ad-hoc for personal use.
    On first launch macOS Gatekeeper may block it — right-click the app and
    choose Open, or run:
      xattr -dr com.apple.quarantine "#{appdir}/Meetily.app"
  EOS
end
