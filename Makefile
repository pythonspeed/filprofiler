libpymemprofile_preload.so: mempreload.c
	gcc -O2 -shared -ldl -g -fPIC -fvisibility=hidden -Wall -o libpymemprofile_preload.so mempreload.c

test:
	env RUST_BACKTRACE=1 PYTHONMALLOC=malloc LD_PRELOAD=./libpymemprofile_preload.so python3.8 example.py
