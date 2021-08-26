# Behavior impacts on NumPy (BLAS), Zarr, BLOSC, OpenMP, numexpr

Fil can't know [which Python code was responsible for allocations in C threads](threading.md).

Therefore, in order to ensure correct memory tracking, Fil disables thread pools in  BLAS (used by NumPy), BLOSC (used e.g. by Zarr), OpenMP, and `numexpr`.
They are all set to use 1 thread, so calls should run in the calling Python thread and everything should be tracked correctly.

This has some costs:

1. This can reduce performance in some cases, since you're doing computation with one CPU instead of many.
2. Insofar as these libraries allocate memory proportional to number of threads, the measured memory usage might be wrong.

Fil does this for the whole program when using `fil-profile run`.
When using the Jupyter kernel, anything run with the `%%filprofile` magic will have thread pools disabled, but other code should run normally.
