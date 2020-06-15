# Fil 0.4.0 (2020-06-15)


### Features

- Fil now helps debug out-of-memory crashes by dumping memory usage at the time of the crash to an SVG. This feature is experimental. ([#48](https://gitlab.com/pythonspeed/filprofiler/-/issues/48))
- Generating the report should run faster. ([#49](https://gitlab.com/pythonspeed/filprofiler/-/issues/49))


# Fil 0.3.3 (2020-06-10)


### Features

- Allocations from the `realloc()` allocation API are now tracked by Fil. ([#43](https://gitlab.com/pythonspeed/filprofiler/-/issues/43))


### Bugfixes

- Fix a bug that corrupted the SVGs. ([#47](https://gitlab.com/pythonspeed/filprofiler/-/issues/47))


# Fil 0.3.2 (2020-06-04)

### Features

- Hovering over a frame now shows the relevant details on top, where it's visible.


# Fil 0.3.1 (2020-05-25)


### Bugfixes

- Command-line arguments after the script/module now work. To make it easier to implement, changed the code so you do `fil-profile run script.py` instead of `fil-profile script.py`. ([#46](https://gitlab.com/pythonspeed/filprofiler/-/issues/46))


# Fil 0.3.0 (2020-05-21)


### Features

- The flame graphs now include the actual code that was responsible for memory use. ([#29](https://gitlab.com/pythonspeed/filprofiler/-/issues/29))
