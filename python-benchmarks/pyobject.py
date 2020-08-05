l = list(range(1000000))


class C:
    pass


l2 = list(C() for _ in range(1000000))
