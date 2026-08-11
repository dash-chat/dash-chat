# Dev-shell ALSA plumbing for voice-note recording (cpal mic capture) on Linux.
#
# The pinned alsa-lib we rpath in would resolve `default` to the system PipeWire
# ALSA plugin, which it cannot dlopen (ABI mismatch), so route `default` through
# PulseAudio instead. It lives in its own file loaded via an appended @hooks
# entry because NixOS drops a competing `default → pipewire` config in
# /etc/alsa/conf.d, which the base hooks load last and would otherwise win.
{ pkgs, lib }:
let
  pulseDefault = pkgs.writeText "pulse-default.conf" ''
    pcm.!default { type pulse }
    ctl.!default { type pulse }
  '';
  asoundConf = pkgs.writeText "asound.conf" ''
    <${pkgs.alsa-lib}/share/alsa/alsa.conf>
    <${pkgs.alsa-plugins}/share/alsa/alsa.conf.d/50-pulseaudio.conf>
    @hooks.1 {
      func load
      files [ "${pulseDefault}" ]
      errors false
    }
  '';
in
{
  # Must be on the app binary’s runtime rpath for cpal capture.
  libraries = [ pkgs.alsa-lib ];

  packages = [
    pkgs.pkg-config
    pkgs.alsa-lib
  ];

  shellHook = lib.optionalString pkgs.stdenv.isLinux ''
    export ALSA_PLUGIN_DIR="${pkgs.alsa-plugins}/lib/alsa-lib"
    export ALSA_CONFIG_PATH="${asoundConf}"
  '';
}
