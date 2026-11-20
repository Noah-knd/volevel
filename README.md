# Volevel

Ultra-lightweight per-application volume mixer, loud-sound limiter, auto-leveler, and app-linked ducker for Windows — built in Rust directly on Windows Core Audio API (WASAPI) with zero virtual audio drivers.

Windows向けの超軽量・仮想オーディオ不要なアプリ別 音量ミキサー＆爆音リミッター・均一化・ダッキング常駐ツールです。

## Download / ダウンロード

Download **`Volevel-Setup-vX.Y.Z.exe`** from [GitHub Releases](https://github.com/Noah-knd/volevel/releases/latest) and run the installer (no administrator privileges required). A standalone portable binary (`volevel-portable-vX.Y.Z.exe`) is also included.

[GitHub Releases](https://github.com/Noah-knd/volevel/releases/latest) から **`Volevel-Setup-vX.Y.Z.exe`** をダウンロードして実行するだけで導入できます（管理者権限不要・アンインストーラー付き）。

## Features / 主な機能

- **Per-App Mixer & 1-Click Mute**: Process-grouped volume control, real `.exe` icons, mouse-wheel adjustment, and bidirectional sync with Windows Volume Mixer.
- **Loud-Sound Limiter**: Fast-attack ceiling limiter with live peak & gain-reduction visualization and smooth release back to base volume.
- **Auto Leveler**: Gentle volume leveling toward a target loudness with a noise gate so silence is never boosted.
- **App-Linked Ducking**: Automatically lowers `Target` apps (music/browser) while a `Trigger` app (voice chat) is active.
- **Failsafe Volume Restore**: Restores every app's Windows volume to its base level on bypass, mode off, hide, or exit.
- **System Tray, Auto-Start & i18n**: Native tray resident with color/grayscale status icon, Windows startup toggle, per-app hide/reset, and instant Japanese/English switching.

## Build from Source

```powershell
cargo test
cargo build --release
```

## License

MIT — see [LICENSE](LICENSE).
