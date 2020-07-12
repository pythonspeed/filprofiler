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
