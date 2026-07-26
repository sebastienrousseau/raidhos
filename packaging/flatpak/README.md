<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Flatpak packaging

The Flatpak is the recommended path for any Linux desktop that
isn't Debian/Ubuntu/Arch/Fedora — specifically:

- Fedora Silverblue / Kinoite / Bazzite
- SteamOS / Bazzite-Deck
- openSUSE Aeon / MicroOS Desktop
- NixOS via `programs.flatpak.enable`
- Pop!_OS, elementary OS, Mint (when the user prefers Flatpak)
- Any other immutable / atomic distro

## Install

```bash
flatpak install --user flathub io.github.sebastienrousseau.raidhos
flatpak run io.github.sebastienrousseau.raidhos
```

(Flathub publish is pending; until then, build locally — see
**Build locally** below.)

## Build locally

```bash
flatpak-builder --user --install --force-clean \
    build-dir packaging/flatpak/io.github.sebastienrousseau.raidhos.yml
```

Then `flatpak run io.github.sebastienrousseau.raidhos`.

## What's in the manifest

| File | Purpose |
|---|---|
| [`io.github.sebastienrousseau.raidhos.yml`](io.github.sebastienrousseau.raidhos.yml) | Flatpak module graph. Pulls in parted + dosfstools + exfatprogs alongside the RaidhOS binaries so the sandbox has the install pipeline's runtime tools. |
| [`io.github.sebastienrousseau.raidhos.desktop`](io.github.sebastienrousseau.raidhos.desktop) | Desktop entry. Sets `StartupWMClass=raidhos-ui` so the Tauri window shows up under the right icon. |
| [`io.github.sebastienrousseau.raidhos.metainfo.xml`](io.github.sebastienrousseau.raidhos.metainfo.xml) | AppStream metadata. Required by Flathub and GNOME Software / KDE Discover. |

## Permissions explained

```yaml
finish-args:
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --share=network
  - --filesystem=host:ro
  - --filesystem=xdg-download
  - --device=all
  - --talk-name=org.freedesktop.PolicyKit1
  - --talk-name=org.freedesktop.UDisks2
  - --talk-name=org.gnupg.gpg-agent
```

- `--device=all` is **not** a backdoor. RaidhOS's job is to write
  a USB stick; this is the only Flatpak permission that lets it
  see raw block devices. The polkit policy still mediates the
  elevation; the sandboxed binary still has to `pkexec` out.
- `--filesystem=host:ro` is read-only by design. Writing only
  happens against block devices, not regular files.
- `--filesystem=xdg-download` makes "drag-drop your ISO" work
  without confusing portal prompts.

If a user is uncomfortable granting `--device=all`, they can
revoke the permission via `flatpak override --user
io.github.sebastienrousseau.raidhos --nodevice=all`, but the
install command will then fail with a clear error.

## What this Flatpak does **not** ship

- A bundled GRUB EFI image. The reproducible build pipeline
  (`tools/grub/`) produces it; the release tarball ships it; the
  Flatpak vendors the *output* of that pipeline at flatpak-build
  time so the Flatpak itself stays focused on the RaidhOS
  binaries.

## Maintainer flow

1. Tag `vX.Y.Z` upstream.
2. Update the `commit:` / `tag:` / SHA256 fields in
   `io.github.sebastienrousseau.raidhos.yml`.
3. Bump `<release version="X.Y.Z" …>` in the metainfo.
4. Push to the Flathub `flathub/io.github.sebastienrousseau.raidhos`
   repository via PR.
