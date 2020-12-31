import sys, resource

if len(sys.argv) == 3:
    # Limit given resource (RLIMIT_DATA or RLIMIT_AS) to number of MB.
    size = int(sys.argv[2]) * 1024 * 1024
    resource.setrlimit(getattr(resource, sys.argv[1]), (size, size))

l = []
for i in range(1_000_000_000_000_000):
    l.append("{} lalalalalala".format(i) * 200)
