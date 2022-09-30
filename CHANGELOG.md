# Release notes

## Fil 2022.09.3 (2022-09-30)

### Bugfixes

- Complex flamegraphs should render faster. ([#427](https://github.com/pythonspeed/filprofiler/issues/427))

## Fil 2022.09.2 (2022-09-29)

### Bugfixes

- Hopefully fixed segfault on macOS, on Python 3.7 and perhaps other versions. ([#412](https://github.com/pythonspeed/filprofiler/issues/412))

## Fil 2022.09.0 (2022-09-12)

### Features

- Added wheels for ARM on Linux (`aarch64`), useful for running native Docker images on ARM Macs. ([#395](https://github.com/pythonspeed/filprofiler/issues/395))

## Fil 2022.07.0 (2022-07-22)

### Bugfixes

- Stopped using `jemalloc` on Linux, for better compatibility with certain libraries. ([#389](https://github.com/pythonspeed/filprofiler/issues/389))
- Speed up rendering of flamegraphs in cases where there are many smaller allocations, by filtering out allocations smaller than 0.2% of total memory. Future releases may re-enable showing smaller allocations if a better fix can be found. ([#390](https://github.com/pythonspeed/filprofiler/issues/390))

## Fil 2022.06.0 (2022-06-19)

### Features

- Added wheels for macOS ARM/Silicon machines. ([#383](https://github.com/pythonspeed/filprofiler/issues/383))

## Fil 2022.05.0 (2022-05-19)

### Bugfixes

- Fix a number of potential deadlock scenarios when writing out reports. ([#374](https://github.com/pythonspeed/filprofiler/issues/374), [#365](https://github.com/pythonspeed/filprofiler/issues/365), [#349](https://github.com/pythonspeed/filprofiler/issues/349))
- Give more accurate message when running in no-browser mode (thanks to Paul-Louis NECH). ([#347](https://github.com/pythonspeed/filprofiler/issues/347))

## Fil 2022.03.0 (2022-03-27)

### Bugfixes

- Don't include memory usage from NumPy imports in the profiling output. This is somewhat inaccurate, but is a reasonable short-term workaround. ([#308](https://github.com/pythonspeed/filprofiler/issues/308))
- Added explanation of why error messages are printed on macOS when opening browser. ([#334](https://github.com/pythonspeed/filprofiler/issues/334))
- The directories where reports are stored now avoid the characters ':' and '.', for better compatibility with other operating systems. ([#336](https://github.com/pythonspeed/filprofiler/issues/336))


### Deprecations and Removals

- Python 3.6 support was dropped. ([#342](https://github.com/pythonspeed/filprofiler/issues/342))


## Fil 2022.01.1 (2022-01-30)


### Bugfixes

- The jemalloc package used on Linux was unmaintained and old, and broke Conda-Forge builds; switched to a newer one. ([#302](https://github.com/pythonspeed/filprofiler/issues/302))


## Fil 2022.01.0 (2022-01-26)


### Features

- Reports now have a "open in new tab" button. Thanks to @petergaultney for the suggestion. ([#298](https://github.com/pythonspeed/filprofiler/issues/298))


### Improved Documentation

- Improved explanations in report of what it is that Fil tracks, and what a flamegraph tells you. ([#185](https://github.com/pythonspeed/filprofiler/issues/185))
- Fix bad command name in the API documentation, thanks to @kdebrab. ([#291](https://github.com/pythonspeed/filprofiler/issues/291))


### Misc

- [#292](https://github.com/pythonspeed/filprofiler/issues/292)


## Fil 2021.12.2 (2021-12-15)


### Bugfixes

- Work on versions of Linux with weird glibc versions. ([#277](https://github.com/pythonspeed/filprofiler/issues/277))


## Fil 2021.12.1 (2021-12-03)


### Features

- Build 3.10 wheels for macOS too. ([#268](https://github.com/pythonspeed/filprofiler/issues/268))


## Fil 2021.12.0 (2021-12-03)


### Features

- Added Python 3.10 support. ([#242](https://github.com/pythonspeed/filprofiler/issues/242))


## Fil 2021.11.1 (2021-11-19)


### Bugfixes

- Added back wheels for macOS Catalina (10.15). ([#253](https://github.com/pythonspeed/filprofiler/issues/253))


## Fil 2021.11.0 (2021-11-08)


### Bugfixes

- Fixed crashes on macOS Monterey. ([#248](https://github.com/pythonspeed/filprofiler/issues/248))


## Fil 2021.09.1 (2021-09-27)


### Bugfixes

- SIGUSR2 previously did not actually dump memory. Thanks to @gaspard-quenard for the bug report. ([#237](https://github.com/pythonspeed/filprofiler/issues/237))


## Fil 2021.9.0 (2021-09-24)


### Bugfixes

- Fix problem on macOS where certain subprocesses (e.g. from Homebrew) would fail to start from Python processes running under Fil. Thanks to @dreid for the bug report. ([#230](https://github.com/pythonspeed/filprofiler/issues/230))


## Fil 2021.8.0 (2021-08-17)


### Bugfixes

- Fix Apache Beam (and other libraries that depend on pickling `__main__` module) when using `filprofile run -m`. ([#202](https://github.com/pythonspeed/filprofiler/issues/202))
- Fixed potential reentrancy bugs; unclear if this had any user-facing impacts, though. ([#215](https://github.com/pythonspeed/filprofiler/issues/215))


## Fil 2021.7.1 (2021-07-18)


### Bugfixes

- Fixed segfault on some Linux versions (regression in release 2021.7.0). ([#208](https://github.com/pythonspeed/filprofiler/issues/208))


## Fil 2021.7.0 (2021-07-12)


### Features

- Added a `--disable-oom-detection` to disable the out-of-memory detection heuristic. ([#201](https://github.com/pythonspeed/filprofiler/issues/201))


### Bugfixes

- When using the Jupyter `%%filprofile` magic, locals defined in the cell are now stored in the Jupyter session as usual. ([#167](https://github.com/pythonspeed/filprofiler/issues/167))
- Emulate Python's module running code more faithfully, to enable profiling things like Apache Beam. ([#202](https://github.com/pythonspeed/filprofiler/issues/202))


## Fil 2021.5.0 (2021-05-06)


### Bugfixes

- Fixed bug where certain allocations went missing during thread creation and cleanup. ([#179](https://github.com/pythonspeed/filprofiler/issues/179))


## Fil 2021.4.4 (2021-04-28)


### Bugfixes

- Fixed race condition in threads that resulting in wrong allocation being removed in the tracking code. ([#175](https://github.com/pythonspeed/filprofiler/issues/175))


## Fil 2021.4.3 (2021-04-15)


### Bugfixes

- **Major bugfix:** mmap() was usually not added correctly on Linux, and when it was, munmap() was ignored. ([#161](https://github.com/pythonspeed/filprofiler/issues/161))


## Fil 2021.4.2 (2021-04-14)


### Features

- Added --no-browser option to disable automatically opening reports in a browser. ([#59](https://github.com/pythonspeed/filprofiler/issues/59))


### Bugfixes

- Fixed bug where aligned_alloc()-created allocations were untracked when using pip packages with Conda; specifically this is relevant to libraries written in C++. ([#152](https://github.com/pythonspeed/filprofiler/issues/152))
- Improved output in the rare case where allocations go missing. ([#154](https://github.com/pythonspeed/filprofiler/issues/154))
- Fixed potential problem with threads noticing profiling is enabled. ([#156](https://github.com/pythonspeed/filprofiler/issues/156))


## Fil 2021.4.1 (2021-04-08)


### Bugfixes

- Fixed bug where reverse SVG sometimes was generated empty, e.g. if source code used tabs. ([#150](https://github.com/pythonspeed/filprofiler/issues/150))


## Fil 2021.4.0 (2021-04-01)

### Bugfixes
- Fil no longer blows up if checking cgroup memory is not possible, e.g. on CentOS 7. ([#147](https://github.com/pythonspeed/filprofiler/issues/147))


## Fil 2021.3.0 (2021-03-19)

### Features

- Try to ensure monospace font is used for reports. ([#143](https://github.com/pythonspeed/filprofiler/issues/143))


### Bugfixes

- Number of allocations in the profiling results are now limited to 10,000. If there are more than this, they are all quite tiny, so probably less informative, and including massive number of tiny allocations makes report generation (and report display) extremely resource intensive. ([#140](https://github.com/pythonspeed/filprofiler/issues/140))
- The out-of-memory detector should work more reliably on Linux. ([#144](https://github.com/pythonspeed/filprofiler/issues/144))

## Fil 0.17.0 (2021-03-02)


### Features

- Improve error messages when using API in subprocesses, so it's clear it's not (yet) possible. ([#133](https://github.com/pythonspeed/filprofiler/issues/133))


## Fil 0.16.0 (2021-02-24)


### Bugfixes

- On Linux, use a more robust method of preloading the shared library (requires glibc 2.30+, i.e. a Linux distribution released in 2020 or later). ([#133](https://github.com/pythonspeed/filprofiler/issues/133))
- Fixed in regression in Fil v0.15 that made it unusable on macOS. ([#135](https://github.com/pythonspeed/filprofiler/issues/135))
- Fewer spurious warnings about launching subprocesses. ([#136](https://github.com/pythonspeed/filprofiler/issues/136))


## Fil 0.15.0 (2021-02-18)


### Features

- Fil now supports profiling individual functions in normal Python scripts; previously this was only possible in Jupyter. ([#71](https://github.com/pythonspeed/filprofiler/issues/71))


### Bugfixes

- Fil now works better with subprocessses. It doesn't support memory tracking in subprocesses yet, but it doesn't break them either. ([#117](https://github.com/pythonspeed/filprofiler/issues/117))


## Fil 0.14.1 (2021-01-15)


### Features

- Report memory stats when out-of-memory event is detected. ([#114](https://github.com/pythonspeed/filprofiler/issues/114))


### Bugfixes

- Correctly handle bad data from cgroups about memory limits, fixing erroneous out-of-memory caused by Docker. ([#113](https://github.com/pythonspeed/filprofiler/issues/113))


## Fil 0.14.0 (2021-01-13)


### Features

- Out-of-memory detection should work in many more cases than before. ([#96](https://github.com/pythonspeed/filprofiler/issues/96))


## Fil 0.13.1 (2020-11-30)


### Features

- Fil now supports Python 3.9. ([#83](https://github.com/pythonspeed/filprofiler/issues/83))


## Fil 0.13.0 (2020-11-27)


### Bugfixes

- Fil no longer uses a vast amount of memory to generate the SVG report. ([#102](https://github.com/pythonspeed/filprofiler/issues/102))


## Fil 0.12.0 (2020-11-21)


### Bugfixes

- Fixed bug that would cause crashes when thread-local storage destructors allocated or freed memory. Thanks to @winash12 for reporting the issue. ([#99](https://github.com/pythonspeed/filprofiler/issues/99))


## Fil 0.11.0 (2020-11-19)

### Features

- Allocations in C threads are now considered allocations by the Python code that launched the thread, to help give some sense of where they came from. ([#72](https://github.com/pythonspeed/filprofiler/issues/72))
- It's now possible to run Fil by doing `python -m filprofiler` in addition to running it as `fil-profile`. ([#82](https://github.com/pythonspeed/filprofiler/issues/82))
- Small performance improvements reducing overhead of malloc()/free() tracking. ([#88](https://github.com/pythonspeed/filprofiler/issues/88) and [#95](https://github.com/pythonspeed/filprofiler/issues/95))


### Bugfixes

- When running in Jupyter, NumPy/BLOSC/etc. thread pools are only limited to one thread when actually running a Fil profile. This means Fil's Jupyter kernel is even closer to running the way a normal Python 3 kernel would. ([#72](https://github.com/pythonspeed/filprofiler/issues/72))


## Fil 0.10.0 (2020-08-28)


### Features

- Added support for profiling inside Jupyter. ([#12](https://github.com/pythonspeed/filprofiler/issues/12))
- Fil can now be installed via Conda-Forge. ([#20](https://github.com/pythonspeed/filprofiler/issues/20))


## Fil 0.9.0 (2020-08-13)


### Features

- When tracking large numbers of allocations, Fil now runs _much_ faster, and has much less memory overhead. ([#65](https://github.com/pythonspeed/filprofiler/issues/65))
- Added support for tracking allocations done using `posix_memalign(3)`. ([#61](https://github.com/pythonspeed/filprofiler/issues/61))

### Bugfixes

- Fixed edge case for large allocations, where wrong number of bytes was recorded as freed. ([#66](https://github.com/pythonspeed/filprofiler/issues/66))


## Fil 0.8.0 (2020-07-24)


### Features

- Switched to using jemalloc on Linux, which should deal better both in terms of memory usage and speed with many small allocations.
  It also simplifies the code. ([#42](https://github.com/pythonspeed/filprofiler/issues/42))
- Further reduced memory overhead for tracking objects, at the cost of slightly lower resolution when tracking allocations >2GB.
  Large allocations >2GB will only be accurate to a resoluion of ~1MB, i.e. they might be off by approximately 0.05%. ([#47](https://github.com/pythonspeed/filprofiler/issues/47))


## Fil 0.7.2 (2020-07-12)


### Bugfixes

- Significantly reduced the memory used to generate the SVG report. ([#38](https://github.com/pythonspeed/filprofiler/issues/38))
- Reduced memory overhead of Fil somewhat, specifically when tracking large numbers of small allocations. ([#43](https://github.com/pythonspeed/filprofiler/issues/43))


## Fil 0.7.1 (2020-07-07)


### Bugfixes

- Fix bug that prevented Fil from running on macOS Mojave and older. ([#36](https://github.com/pythonspeed/filprofiler/issues/36))


## Fil 0.7.0 (2020-07-03)


### Features

- C++ allocations get tracked more reliably, especially on macOS. ([#10](https://github.com/pythonspeed/filprofiler/issues/10))
- Validated that Fortran 90 allocations are tracked by Fil. ([#11](https://github.com/pythonspeed/filprofiler/issues/11))


### Misc

- [#26](https://github.com/pythonspeed/filprofiler/issues/26)


## Fil 0.6.0 (2020-07-01)


### Features

- Anonymous mmap()s are now tracked by Fil. ([#29](https://github.com/pythonspeed/filprofiler/issues/29))


## Fil 0.5.0 (2020-06-22)


### Features

- macOS is now supported. ([#15](https://github.com/pythonspeed/filprofiler/issues/15))


### Bugfixes

- Running `fil-profile` with no arguments now prints the help. ([#21](https://github.com/pythonspeed/filprofiler/issues/21))


## Fil 0.4.0 (2020-06-15)


### Features

- Fil now helps debug out-of-memory crashes by dumping memory usage at the time of the crash to an SVG. This feature is experimental.
- Generating the report should run faster.


## Fil 0.3.3 (2020-06-10)


### Features

- Allocations from the `realloc()` allocation API are now tracked by Fil.


### Bugfixes

- Fix a bug that corrupted the SVGs.


## Fil 0.3.2 (2020-06-04)

### Features

- Hovering over a frame now shows the relevant details on top, where it's visible.


## Fil 0.3.1 (2020-05-25)


### Bugfixes

- Command-line arguments after the script/module now work. To make it easier to implement, changed the code so you do `fil-profile run script.py` instead of `fil-profile script.py`.


## Fil 0.3.0 (2020-05-21)


### Features

- The flame graphs now include the actual code that was responsible for memory use.
