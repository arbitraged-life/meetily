# Homebrew Cask — Meetily

Install the personal fork build of Meetily via Homebrew.

## Install

This repo doubles as a Homebrew tap. Once a `v<version>` GitHub release with the
`meetily_<version>_aarch64.dmg` asset is published:

```sh
brew tap arbitraged-life/meetily https://github.com/arbitraged-life/meetily
brew install --cask meetily
```

Or one-shot from the raw cask file:

```sh
brew install --cask https://raw.githubusercontent.com/arbitraged-life/meetily/main/Casks/meetily.rb
```

## Notes

- Apple Silicon only (ad-hoc signed for personal use).
- First launch may be blocked by Gatekeeper — right-click → Open, or
  `xattr -dr com.apple.quarantine "/Applications/meetily.app"`.

## Releasing a new version

After `./build-and-install.sh` produces the DMG:

```sh
V=0.3.0
DMG="target/release/bundle/dmg/meetily_${V}_aarch64.dmg"
shasum -a 256 "$DMG"          # update sha256 in Casks/meetily.rb
gh release create "v${V}" "$DMG" --repo arbitraged-life/meetily --title "v${V}"
```

Then bump `version`/`sha256` in `Casks/meetily.rb` and commit. Validate with:

```sh
brew style --cask Casks/meetily.rb
```
