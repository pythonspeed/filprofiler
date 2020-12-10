# Fil 0.13.1 (2020-11-30)


### Features

- Fil now supports Python 3.9. ([#83](https://github.com/pythonspeed/filprofiler/issues/83))


# Fil 0.13.0 (2020-11-27)


### Bugfixes

- Fil no longer uses a vast amount of memory to generate the SVG report. ([#102](https://github.com/pythonspeed/filprofiler/issues/102))


# Fil 0.12.0 (2020-11-21)


### Bugfixes

- Fixed bug that would cause crashes when thread-local storage destructors allocated or freed memory. Thanks to @winash12 for reporting the issue. ([#99](https://github.com/pythonspeed/filprofiler/issues/99))


# Fil 0.11.0 (2020-11-19)

### Features

- Allocations in C threads are now considered allocations by the Python code that launched the thread, to help give some sense of where they came from. ([#72](https://github.com/pythonspeed/filprofiler/issues/72))
- It's now possible to run Fil by doing `python -m filprofiler` in addition to running it as `fil-profile`. ([#82](https://github.com/pythonspeed/filprofiler/issues/82))
- Small performance improvements reducing overhead of malloc()/free() tracking. ([#88](https://github.com/pythonspeed/filprofiler/issues/88) and [#95](https://github.com/pythonspeed/filprofiler/issues/95))


### Bugfixes

- When running in Jupyter, NumPy/BLOSC/etc. thread pools are only limited to one thread when actually running a Fil profile. This means Fil's Jupyter kernel is even closer to running the way a normal Python 3 kernel would. ([#72](https://github.com/pythonspeed/filprofiler/issues/72))


# Fil 0.10.0 (2020-08-28)


### Features

- Added support for profiling inside Jupyter. ([#12](https://github.com/pythonspeed/filprofiler/issues/12))
- Fil can now be installed via Conda-Forge. ([#20](https://github.com/pythonspeed/filprofiler/issues/20))


# Fil 0.9.0 (2020-08-13)


### Features

- When tracking large numbers of allocations, Fil now runs _much_ faster, and has much less memory overhead. ([#65](https://github.com/pythonspeed/filprofiler/issues/65))
- Added support for tracking allocations done using `posix_memalign(3)`. ([#61](https://github.com/pythonspeed/filprofiler/issues/61))

### Bugfixes

- Fixed edge case for large allocations, where wrong number of bytes was recorded as freed. ([#66](https://github.com/pythonspeed/filprofiler/issues/66))


# Fil 0.8.0 (2020-07-24)


### Features

- Switched to using jemalloc on Linux, which should deal better both in terms of memory usage and speed with many small allocations.
  It also simplifies the code. ([#42](https://github.com/pythonspeed/filprofiler/issues/42))
- Further reduced memory overhead for tracking objects, at the cost of slightly lower resolution when tracking allocations >2GB.
  Large allocations >2GB will only be accurate to a resoluion of ~1MB, i.e. they might be off by approximately 0.05%. ([#47](https://github.com/pythonspeed/filprofiler/issues/47))


# Fil 0.7.2 (2020-07-12)


### Bugfixes

- Significantly reduced the memory used to generate the SVG report. ([#38](https://github.com/pythonspeed/filprofiler/issues/38))
- Reduced memory overhead of Fil somewhat, specifically when tracking large numbers of small allocations. ([#43](https://github.com/pythonspeed/filprofiler/issues/43))


# Fil 0.7.1 (2020-07-07)


### Bugfixes

- Fix bug that prevented Fil from running on macOS Mojave and older. ([#36](https://github.com/pythonspeed/filprofiler/issues/36))


# Fil 0.7.0 (2020-07-03)


### Features

- C++ allocations get tracked more reliably, especially on macOS. ([#10](https://github.com/pythonspeed/filprofiler/issues/10))
- Validated that Fortran 90 allocations are tracked by Fil. ([#11](https://github.com/pythonspeed/filprofiler/issues/11))


### Misc

- [#26](https://github.com/pythonspeed/filprofiler/issues/26)


# Fil 0.6.0 (2020-07-01)


### Features

- Anonymous mmap()s are now tracked by Fil. ([#29](https://github.com/pythonspeed/filprofiler/issues/29))


# Fil 0.5.0 (2020-06-22)


### Features

- macOS is now supported. ([#15](https://github.com/pythonspeed/filprofiler/issues/15))


### Bugfixes

- Running `fil-profile` with no arguments now prints the help. ([#21](https://github.com/pythonspeed/filprofiler/issues/21))


# Fil 0.4.0 (2020-06-15)


### Features

- Fil now helps debug out-of-memory crashes by dumping memory usage at the time of the crash to an SVG. This feature is experimental.
- Generating the report should run faster.


# Fil 0.3.3 (2020-06-10)


### Features

- Allocations from the `realloc()` allocation API are now tracked by Fil.


### Bugfixes

- Fix a bug that corrupted the SVGs.


# Fil 0.3.2 (2020-06-04)

### Features

- Hovering over a frame now shows the relevant details on top, where it's visible.


# Fil 0.3.1 (2020-05-25)


### Bugfixes

- Command-line arguments after the script/module now work. To make it easier to implement, changed the code so you do `fil-profile run script.py` instead of `fil-profile script.py`.


# Fil 0.3.0 (2020-05-21)


### Features

- The flame graphs now include the actual code that was responsible for memory use.
