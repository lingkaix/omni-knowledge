#!/usr/bin/env bash
# Run one packaged spike binary under a private Xvfb and ibus home.
# Refuses a display that is already set so it cannot drive a user session.
set -euo pipefail

if [[ "${OMNI_SHELL_SPIKE_OBSERVE:-}" != 1 ]]; then
  echo "linux-observe: set OMNI_SHELL_SPIKE_OBSERVE=1 in the isolated job" >&2
  exit 2
fi
if [[ -n "${DISPLAY:-}" ]]; then
  echo "linux-observe: DISPLAY is already set; refusing" >&2
  exit 2
fi
mode="${1:-}"
binary="${2:-}"
out_dir="${3:-}"
case "$mode" in
  probe|ime|drop|dialog) ;;
  *)
    echo "linux-observe: mode must be probe, ime, drop, or dialog" >&2
    exit 2
    ;;
esac
if [[ ! -x "$binary" ]]; then
  echo "linux-observe: binary is not executable: $binary" >&2
  exit 2
fi
mkdir -p "$out_dir"
note="$out_dir/$mode.observer.txt"
log="$out_dir/$mode.log"
evidence="$out_dir/$mode.json"
: >"$note"

work="$(mktemp -d)"
cleanup() {
  if [[ -n "${app_pid:-}" ]]; then
    kill -TERM "-$app_pid" 2>/dev/null || kill -TERM "$app_pid" 2>/dev/null || true
  fi
  if [[ -n "${drag_pid:-}" ]]; then
    kill -TERM "$drag_pid" 2>/dev/null || true
  fi
  if [[ -n "${ibus_pid:-}" ]]; then
    kill -TERM "$ibus_pid" 2>/dev/null || true
  fi
  if [[ -n "${openbox_pid:-}" ]]; then
    kill -TERM "$openbox_pid" 2>/dev/null || true
  fi
  if [[ -n "${dbus_pid:-}" ]]; then
    kill -TERM "$dbus_pid" 2>/dev/null || true
  fi
  if [[ -n "${xvfb_pid:-}" ]]; then
    kill -TERM "$xvfb_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
on_exit() {
  echo "----- ${mode} observer -----"
  cat "$note" 2>/dev/null || true
  echo "----- ${mode} binary log -----"
  cat "$log" 2>/dev/null || true
  echo "----- ${mode} xvfb -----"
  tail -n 40 "$out_dir/$mode.xvfb.log" 2>/dev/null || true
  echo "----- ${mode} ibus -----"
  tail -n 40 "$out_dir/$mode.ibus.log" 2>/dev/null || true
  echo "----- ${mode} engines -----"
  cat "$out_dir/$mode.engines.txt" 2>/dev/null || true
  echo "----- ${mode} dbus -----"
  tail -n 40 "$out_dir/$mode.dbus.log" 2>/dev/null || true
  cleanup
}
trap on_exit EXIT

export HOME="$work/home"
export XDG_CONFIG_HOME="$work/config"
export XDG_CACHE_HOME="$work/cache"
export XDG_RUNTIME_DIR="$work/run"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
export DISPLAY=:99
export GDK_BACKEND=x11
export GSK_RENDERER=cairo
export LIBGL_ALWAYS_SOFTWARE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
# The runner refuses bwrap's uid map. WebKit then aborts with
# "Failed to fully launch dbus-proxy". Observer process only.
export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1
export GTK_A11Y=none
export GTK_IM_MODULE=ibus
export QT_IM_MODULE=ibus
export XMODIFIERS=@im=ibus
export OMNI_SHELL_SPIKE_MODE="$mode"
export OMNI_SHELL_SPIKE_EVIDENCE="$evidence"

if ! command -v Xvfb >/dev/null; then
  echo "prerequisite missing: Xvfb" | tee -a "$note"
  exit 1
fi
Xvfb :99 -screen 0 1280x800x24 >"$out_dir/$mode.xvfb.log" 2>&1 &
xvfb_pid=$!
sleep 1
echo "display=$DISPLAY xvfb_pid=$xvfb_pid" >>"$note"

if command -v openbox >/dev/null; then
  openbox >"$out_dir/$mode.openbox.log" 2>&1 &
  openbox_pid=$!
  sleep 0.5
  echo "openbox_pid=$openbox_pid" >>"$note"
else
  echo "prerequisite missing: openbox" >>"$note"
fi

if command -v dbus-daemon >/dev/null; then
  dbus-daemon --session --address="unix:path=$XDG_RUNTIME_DIR/bus" --nofork --nopidfile --nosyslog >>"$out_dir/$mode.dbus.log" 2>&1 &
  dbus_pid=$!
  export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
  sleep 0.5
  echo "dbus_pid=$dbus_pid address=$DBUS_SESSION_BUS_ADDRESS" >>"$note"
else
  echo "prerequisite missing: dbus-daemon" >>"$note"
fi

if command -v ibus-daemon >/dev/null; then
  ibus-daemon -drx --panel disable >"$out_dir/$mode.ibus.log" 2>&1 &
  ibus_pid=$!
  sleep 1
  if command -v ibus >/dev/null; then
    ibus list-engine >"$out_dir/$mode.engines.txt" 2>&1 || true
    selected=0
    for _ in 1 2 3 4 5; do
      if ibus engine libpinyin >>"$out_dir/$mode.ibus.log" 2>&1; then
        selected=1
        break
      fi
      sleep 0.4
    done
    if [[ "$selected" == 1 ]]; then
      echo "ibus engine=libpinyin" >>"$note"
    else
      echo "ibus engine libpinyin failed" >>"$note"
    fi
  fi
  echo "ibus_pid=${ibus_pid:-absent}" >>"$note"
else
  echo "prerequisite missing: ibus-daemon" >>"$note"
fi

setsid "$binary" >"$log" 2>&1 &
app_pid=$!
echo "app_pid=$app_pid binary=$binary" >>"$note"

ready=0
for _ in $(seq 1 90); do
  if grep -q "spike-listening" "$log"; then
    ready=1
    break
  fi
  if ! kill -0 "$app_pid" 2>/dev/null; then
    echo "binary exited before spike-listening" >>"$note"
    break
  fi
  sleep 1
done
if [[ "$ready" != 1 ]]; then
  echo "readiness failed" >>"$note"
  exit 1
fi

window_id=""
focus_window() {
  local id
  id="$(xdotool search --name "Omni Shell Spike" 2>/dev/null | head -n 1 || true)"
  window_id="$id"
  if [[ -n "$id" ]]; then
    xdotool windowfocus --sync "$id" >>"$note" 2>&1 || true
    xdotool windowactivate --sync "$id" >>"$note" 2>&1 || true
    echo "window=$id" >>"$note"
  else
    echo "window not found" >>"$note"
  fi
}
key_to_window() {
  if [[ -n "$window_id" ]]; then
    xdotool key --window "$window_id" "$@" >>"$note" 2>&1 || echo "key failed: $*" >>"$note"
  else
    echo "no window for key: $*" >>"$note"
  fi
}

if ! command -v xdotool >/dev/null; then
  echo "prerequisite missing: xdotool" >>"$note"
else
  sleep 1
  focus_window
  case "$mode" in
    probe)
      key_to_window ctrl+shift+m
      ;;
    ime)
      xdotool mousemove 440 80 click 1 >>"$note" 2>&1 || true
      sleep 0.4
      # XTEST, not XSendEvent. The previous --window send died with BadWindow
      # when the process closed on the first composition update.
      xdotool key --delay 120 n i h a o space >>"$note" 2>&1 || echo "xtest key failed: nihao" >>"$note"
      sleep 2
      xdotool mousemove 120 680 click 1 >>"$note" 2>&1 || true
      sleep 0.4
      xdotool key --delay 120 n i space >>"$note" 2>&1 || echo "xtest key failed: native" >>"$note"
      ;;
    drop)
      drop_file="$work/spike-drop.txt"
      drag_log="$out_dir/$mode.drag.log"
      python3 "$(dirname "$0")/gtk-drag-source.py" "$drop_file" >"$drag_log" 2>&1 &
      drag_pid=$!
      sleep 2
      if ! kill -0 "$drag_pid" 2>/dev/null; then
        echo "gtk drag source exited" >>"$note"
      fi
      drag_id="$(xdotool search --name "Spike Drag Source" 2>/dev/null | head -n 1 || true)"
      if [[ -n "$drag_id" ]]; then
        xdotool windowmove "$drag_id" 1000 40 >>"$note" 2>&1 || true
        xdotool mousemove --window "$drag_id" 90 90 >>"$note" 2>&1 || true
        xdotool mousedown 1 >>"$note" 2>&1 || true
        xdotool mousemove 120 280 >>"$note" 2>&1 || true
        xdotool mouseup 1 >>"$note" 2>&1 || true
        echo "drag_window=$drag_id" >>"$note"
      else
        echo "drag source window was not found" >>"$note"
      fi
      ;;
    dialog)
      sleep 2
      key_to_window Escape
      ;;
  esac
fi

case "$mode" in
  probe) deadline=70 ;;
  ime) deadline=55 ;;
  drop) deadline=35 ;;
  dialog) deadline=25 ;;
esac
for _ in $(seq 1 "$deadline"); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    break
  fi
  sleep 1
done
if kill -0 "$app_pid" 2>/dev/null; then
  echo "deadline exceeded; stopping the binary" >>"$note"
  kill -TERM "-$app_pid" 2>/dev/null || kill -TERM "$app_pid" 2>/dev/null || true
  sleep 1
fi
wait "$app_pid" || echo "binary exit=$?" >>"$note"
app_pid=""

if [[ ! -f "$evidence" ]]; then
  echo "evidence file missing: $evidence" >>"$note"
  exit 1
fi
echo "evidence=$evidence" >>"$note"
