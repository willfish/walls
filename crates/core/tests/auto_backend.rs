use walls_core::apply::{auto_backend_for_desktop, Desktop};
use walls_core::config::ApplyBackendSetting;

#[test]
fn auto_backend_selects_native_desktop_backends_before_fallback() {
    assert_eq!(
        auto_backend_for_desktop(Desktop::Cosmic),
        ApplyBackendSetting::Cosmic
    );
    assert_eq!(
        auto_backend_for_desktop(Desktop::Gnome),
        ApplyBackendSetting::Gnome
    );
    assert_eq!(
        auto_backend_for_desktop(Desktop::Unity),
        ApplyBackendSetting::Gnome
    );
    assert_eq!(
        auto_backend_for_desktop(Desktop::Budgie),
        ApplyBackendSetting::Gnome
    );
    assert_eq!(
        auto_backend_for_desktop(Desktop::Kde),
        ApplyBackendSetting::Kde
    );
    assert_eq!(
        auto_backend_for_desktop(Desktop::Xfce),
        ApplyBackendSetting::Xfce
    );
    assert_eq!(
        auto_backend_for_desktop(Desktop::Sway),
        ApplyBackendSetting::Sway
    );
    assert_eq!(
        auto_backend_for_desktop(Desktop::Hyprland),
        ApplyBackendSetting::Hyprland
    );
}

#[test]
fn auto_backend_leaves_unsupported_desktops_for_feh_fallback() {
    for desktop in [
        Desktop::Lxde,
        Desktop::Lxqt,
        Desktop::Mate,
        Desktop::Cinnamon,
        Desktop::Lingmo,
        Desktop::Deepin,
        Desktop::Trinity,
        Desktop::Fluxbox,
        Desktop::Enlightenment,
        Desktop::Awesome,
        Desktop::Unknown,
    ] {
        assert_eq!(
            auto_backend_for_desktop(desktop),
            ApplyBackendSetting::Auto,
            "{desktop:?}"
        );
    }
}
