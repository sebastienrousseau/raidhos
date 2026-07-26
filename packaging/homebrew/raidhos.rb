class Raidhos < Formula
  desc "Memory-safe, multi-ISO USB imager (CLI). The privileged helper is bundled."
  homepage "https://github.com/sebastienrousseau/raidhos"
  license "GPL-3.0-only"
  version "0.0.1"

  # cosign-verified release artefacts. After cutting v0.0.1, replace the
  # placeholder URLs with the actual release URLs and sha256s from
  # https://github.com/sebastienrousseau/raidhos/releases.
  on_macos do
    on_arm do
      url "https://github.com/sebastienrousseau/raidhos/releases/download/v#{version}/raidhos-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256"
    end
    on_intel do
      url "https://github.com/sebastienrousseau/raidhos/releases/download/v#{version}/raidhos-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/sebastienrousseau/raidhos/releases/download/v#{version}/raidhos-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256"
    end
    on_intel do
      url "https://github.com/sebastienrousseau/raidhos/releases/download/v#{version}/raidhos-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256"
    end
  end

  def install
    bin.install "raidhos-cli"
    bin.install "raidhos-priv-helper"

    # Man page and shell completions, if shipped in the tarball.
    if (buildpath/"raidhos-cli.1").exist?
      man1.install "raidhos-cli.1"
    end
    bash_completion.install "raidhos-cli.bash" => "raidhos-cli" if (buildpath/"raidhos-cli.bash").exist?
    zsh_completion.install "_raidhos-cli" if (buildpath/"_raidhos-cli").exist?
    fish_completion.install "raidhos-cli.fish" if (buildpath/"raidhos-cli.fish").exist?
  end

  def caveats
    <<~EOS
      RaidhOS will refuse to operate on internal disks. On macOS the
      destructive install path is still being wired up (v0.0.2 target);
      `list-disks` and dry-runs work today.

      Verify this formula's release against the cosign signature at:
        https://github.com/sebastienrousseau/raidhos/releases/v#{version}

      Threat model: https://github.com/sebastienrousseau/raidhos/blob/main/docs/THREAT_MODEL.md
    EOS
  end

  test do
    system "#{bin}/raidhos-cli", "--version"
  end
end
