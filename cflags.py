import sysconfig
import sys

print(
    'export CFLAGS="{} -I{} -I{}"'.format(
        sysconfig.get_config_var("CFLAGS"),
        sysconfig.get_path("include"),
        sysconfig.get_path("platinclude"),
    )
)
