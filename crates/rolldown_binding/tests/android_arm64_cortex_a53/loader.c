#include <dlfcn.h>
#include <stdio.h>

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: %s <shared-object>\n", argv[0]);
    return 2;
  }

  void *handle = dlopen(argv[1], RTLD_LAZY | RTLD_LOCAL);
  if (handle == NULL) {
    fprintf(stderr, "dlopen(%s) failed: %s\n", argv[1], dlerror());
    return 1;
  }

  printf("loaded %s\n", argv[1]);
  return 0;
}
