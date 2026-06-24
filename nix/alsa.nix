# Dev-shell ALSA plumbing for voice-note recording (cpal mic capture) on Linux.
#
# The dev shell rpaths a pinned alsa-lib into the app binary. That alsa-lib
# would otherwise resolve the `default` capture device to the *system* PipeWire
# ALSA plugin, which it can't dlopen (ABI mismatch) — surfacing as
# `snd_pcm_open … 'No such device or address' (ENXIO)`. So route `default`
# through PulseAudio (PipeWire-pulse over its socket) instead, which needs no
# plugin to load.
#
# The base alsa.conf's @hooks load /etc/alsa/conf.d *last*, where NixOS'
# `services.pipewire.alsa` drops a `default → pipewire` config that would
# override a plain top-level `pcm.!default`. So our pulse default lives in its
# own file, loaded via an appended @hooks entry (index 1) that runs after the
# base hooks and therefore wins.
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
  # alsa-lib must be on the app binary's runtime rpath for cpal capture.
  libraries = [ pkgs.alsa-lib ];

  # Build-time discovery of ALSA for the cpal `alsa-sys` crate.
  packages = [
    pkgs.pkg-config
    pkgs.alsa-lib
  ];

  # Dev-shell exports (Linux only) that point cpal's `default` at PulseAudio.
  shellHook = lib.optionalString pkgs.stdenv.isLinux ''
    export ALSA_PLUGIN_DIR="${pkgs.alsa-plugins}/lib/alsa-lib"
    export ALSA_CONFIG_PATH="${asoundConf}"
  '';
}
