"""
Generate lots of callstacks with lots of new peaks.

Don't use loops in order to maximize callstacks.
"""

L = []


def f():
    g()
    g()
    g()
    g()
    g()
    g()
    g()
    g()
    g()
    g()
    g()
    g()
    # Recursion instead of a loop, so we get more callstack IDs.
    if len(L) < 100_000:
        f()


def g():
    h()
    h()
    h()
    h()
    h()
    h()
    h()
    h()
    h()
    h()
    h()
    h()


def h():
    # Increase allocated memory, and also deallocate some memory to trigger
    # check for new peaks()
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x
    L.append(list())
    x = list()
    del x


if __name__ == "__main__":
    f()
