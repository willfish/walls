# Apply Backends

| Backend | Auto-detected desktops | Command path | Manual test |
| --- | --- | --- | --- |
| COSMIC | `XDG_CURRENT_DESKTOP=COSMIC` | `cosmic-ext-bg` or COSMIC config patch | `XDG_CURRENT_DESKTOP=COSMIC walls next` |
| GNOME family | GNOME, Unity, Budgie | `gsettings set org.gnome.desktop.background picture-uri`, `picture-uri-dark`, and `picture-options` | `XDG_CURRENT_DESKTOP=GNOME walls next` |
| KDE Plasma | KDE | `dbus-send --dest=org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript` | `XDG_CURRENT_DESKTOP=KDE walls next` in a Plasma session |
| XFCE | XFCE | `xfconf-query -c xfce4-desktop -p /backdrop ...` existing per-monitor `image-path` / `last-image`, with `xrandr` monitor creation fallback | `XDG_CURRENT_DESKTOP=XFCE walls next` in an XFCE session |
| Sway | Sway | `swaymsg output * bg <path> <mode>` | `XDG_CURRENT_DESKTOP=sway walls next` in a Sway session |
| wlroots compositors | Explicit `apply.backend=wlroots` | restart `swaybg -i <path> -m <mode>` | `apply.backend=wlroots walls next` with `swaybg` installed |
| Hyprland | Hyprland | `hyprctl monitors` text parsing, then restart per-output `swaybg -o <output> -i <path> -m <mode>` | `XDG_CURRENT_DESKTOP=Hyprland walls next` in Hyprland with `swaybg` installed |
| feh/nitrogen fallback | Everything else when `apply.backend=auto` | `feh --bg-fill`, then `nitrogen --set-zoom-fill --save` | `walls next` on an X11 session with `feh` or `nitrogen` installed |
| Custom script | Explicit `apply.backend=custom-script` | `apply.custom_script` | Configure a script and run `walls next` |

Set an explicit backend with `apply.backend` when auto-detection is not appropriate.
