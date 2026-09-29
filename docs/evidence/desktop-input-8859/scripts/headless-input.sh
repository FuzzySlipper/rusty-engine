#!/bin/sh
# A private headless KDE compositor for window tests, with X11 through its own
# Xwayland, on its own D-Bus session so screenshots never touch a desktop user.
#
#   setsid dbus-run-session ./headless-compositor.sh <state-dir> &
#   . <state-dir>/env.sh   # WAYLAND_DISPLAY and DBUS_SESSION_BUS_ADDRESS
#   spectacle -b -n -f -o shot.png
#
# X11 clients use the next free display (see /tmp/.X11-unix). --no-lockscreen
# matters: the lock screen follows the login session's lock state and
# throttles the compositor to about 20 Hz behind it.
state=${1:?state dir}
mkdir -p "$state"
cat > "$state/env.sh" <<ENV
export WAYLAND_DISPLAY=wayland-input
export DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS
export QT_QPA_PLATFORM=wayland
unset DISPLAY
ENV
export KWIN_SCREENSHOT_NO_PERMISSION_CHECKS=1
export KWIN_WAYLAND_NO_PERMISSION_CHECKS=1
exec kwin_wayland --virtual --socket wayland-input --xwayland --no-lockscreen --width 1280 --height 800
