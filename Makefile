libpymemprofile_preload.so: mempreload.c
	gcc -shared -ldl -g -fPIC -Wall -fvisibility=hidden -o libpymemprofile_preload.so mempreload.c

libpymemprofile_shim.so: memshim.c
	gcc -shared -ldl -g -fPIC -Wall -fvisibility=hidden -o libpymemprofile_shim.so memshim.c
