/*
 * linuxdeploy shim: exec the real linuxdeploy with extra --exclude-library
 * flags so host-coupled libraries are never bundled into the AppImage. Two
 * stacks are excluded:
 *   - the wayland client stack (prevents blank-screen EGL mismatch), and
 *   - the glib/gio stack. GIO loads modules from the host (gvfs, gsettings
 *     backends) that are built against the host's glib. AppRun puts bundled
 *     libs first, so a runner-built (older) libglib gets paired with a newer
 *     host gvfs module -> "undefined symbol: g_task_set_static_name". Letting
 *     glib resolve from the host keeps it in lockstep with those modules.
 *
 * tauri-bundler invokes linuxdeploy with a hardcoded argument list and offers
 * no way to pass --exclude-library. It caches the tool at
 * ~/.cache/tauri/linuxdeploy-<arch>.AppImage and, if that path already exists,
 * skips the download and runs it as-is. CI pre-places this compiled shim there
 * (real linuxdeploy moved aside to *.real.AppImage) so every bundler run gets
 * the exclusions. A compiled ELF is required rather than a shell wrapper:
 * tauri zeroes bytes 8-10 of the cached tool (`dd seek=8 count=3`), which are
 * EI_ABIVERSION + padding in an ELF header (already zero, harmless) but would
 * corrupt a script's shebang.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
  const char *home = getenv("HOME");
  if (!home) {
    fprintf(stderr, "linuxdeploy-exclude-wayland: HOME is not set\n");
    return 1;
  }

  const char *arch = getenv("ARCH");
  if (!arch || !*arch) {
    arch = "x86_64";
  }

  char real[4096];
  snprintf(real, sizeof(real), "%s/.cache/tauri/linuxdeploy-%s.real.AppImage",
           home, arch);

  static const char *excludes[] = {
      "--exclude-library=libwayland-client.so*",
      "--exclude-library=libwayland-egl.so*",
      "--exclude-library=libwayland-cursor.so*",
      "--exclude-library=libwayland-server.so*",
      "--exclude-library=libglib-2.0.so*",
      "--exclude-library=libgio-2.0.so*",
      "--exclude-library=libgobject-2.0.so*",
      "--exclude-library=libgmodule-2.0.so*",
  };
  const int n_excludes = (int)(sizeof(excludes) / sizeof(excludes[0]));

  int total = 1 + (argc - 1) + n_excludes + 1;
  char **args = calloc((size_t)total, sizeof(char *));
  if (!args) {
    perror("linuxdeploy-exclude-wayland: calloc");
    return 1;
  }

  int i = 0;
  args[i++] = real;
  for (int j = 1; j < argc; j++) {
    args[i++] = argv[j];
  }
  for (int j = 0; j < n_excludes; j++) {
    args[i++] = (char *)excludes[j];
  }
  args[i] = NULL;

  execv(real, args);
  perror("linuxdeploy-exclude-wayland: execv");
  return 127;
}
