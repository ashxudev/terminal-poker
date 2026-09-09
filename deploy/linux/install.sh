#!/usr/bin/env bash
# Run inside an extracted release bundle. Does not start/stop games implicitly.
set -euo pipefail
cd -- "$(dirname -- "$0")"
sha256sum --check SHA256SUMS
if systemctl --user is-active --quiet poker-server.service; then
    echo 'Stop the server during a maintenance window before installing.' >&2
    exit 1
fi
root="$HOME/.local/share/sneakyblinders"
release="$(sha256sum SHA256SUMS | cut -c1-16)"
destination="$root/releases/$release"
umask 077
mkdir -p "$root/releases" "$root/state" "$HOME/.config/systemd/user"
if [[ -e "$destination" ]]; then
    (cd "$destination" && sha256sum --check SHA256SUMS)
else
    mkdir "$destination"
    install -m 755 poker-server "$destination/poker-server"
    install -m 644 SHA256SUMS source-manifest.json toolchain.txt runtime-libraries.txt poker-server.service "$destination/"
    install -m 755 install.sh "$destination/install.sh"
fi
if [[ -e "$root/current" && ! -L "$root/current" ]]; then
    echo 'Refusing to replace a non-symlink current path.' >&2
    exit 1
fi
previous="$(readlink "$root/current" || true)"
if [[ -n "$previous" && "$previous" != "releases/$release" ]]; then
    printf '%s\n' "$previous" > "$root/previous-release"
fi
ln -s "releases/$release" "$root/current.new.$$"
mv -Tf "$root/current.new.$$" "$root/current"
install -m 600 poker-server.service "$HOME/.config/systemd/user/poker-server.service"
systemctl --user daemon-reload
printf 'Installed %s. Start with: systemctl --user enable --now poker-server\n' "$release"
