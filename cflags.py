import sysconfig
import sys

CFLAGS = "{} -I{} -I{} -fno-omit-frame-pointer".format(
    sysconfig.get_config_var("CFLAGS"),
    sysconfig.get_path("include"),
    sysconfig.get_path("platinclude"),
)

if __name__ == "__main__":
    print(f'export CFLAGS="{CFLAGS}"')
