import resource, time

start = time.time()

l = list(range(1000000))


class C:
    pass


l2 = list(C() for _ in range(1000000))


print("MEMORY USAGE", resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
print("TIME", time.time() - start)
