# Apply Backends

| Backend | Auto-detected desktops | Command path | Manual test |
| --- | --- | --- | --- |
| COSMIC | `XDG_CURRENT_DESKTOP=COSMIC` | `cosmic-ext-bg` or COSMIC config patch | `XDG_CURRENT_DESKTOP=COSMIC walls next` |
| GNOME family | GNOME, Unity, Budgie | `gsettings set org.gnome.desktop.background picture-uri`, `picture-uri-dark`, and `picture-options` | `XDG_CURRENT_DESKTOP=GNOME walls next` |
| KDE Plasma | Explicit `apply.backend=kde` | `dbus-send --dest=org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript` | `apply.backend=kde walls next` in a Plasma session |
| feh/nitrogen fallback | Everything else when `apply.backend=auto` | `feh --bg-fill`, then `nitrogen --set-zoom-fill --save` | `walls next` on an X11 session with `feh` or `nitrogen` installed |
| Custom script | Explicit `apply.backend=custom-script` | `apply.custom_script` | Configure a script and run `walls next` |

Set an explicit backend with `apply.backend` when auto-detection is not appropriate.
