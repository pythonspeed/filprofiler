libpymemprofile_preload.so: mempreload.c
	gcc -shared -ldl -g -fPIC -fvisibility=hidden -Wall -o libpymemprofile_preload.so mempreload.c
