import sysconfig
import sys

print(
    'export CFLAGS="{} -I{} -I{}"'.format(
        sysconfig.get_config_var("CFLAGS"),
        sysconfig.get_path("include"),
        sysconfig.get_path("platinclude"),
    )
)

if len(sys.argv) == 2 and sys.argv[1] == "--link":
    print(f"""export RUSTFLAGS="-Clink-arg=-Wl,-lpython3.{sys.version_info.minor}" """)
