# Profiling a subset of your Python program

Sometimes you only want to profile your Python program part of the time.
For this use case, Fil provides a Python API.

> **Important:** This API turns profiling on and off for the whole process!
> If you want more fine grained profiling, e.g. per thread, please [file an issue](https://github.com/pythonspeed/filprofiler/issues/new).

## Using the Python API

#### 1. Add profiling in your code

Let's you have some code that does the following:

```python
def main():
    config = load_config()
    result = run_processing(config)
    generate_report(result)
```

You only want to get memory profiling for the `run_processing()` call.

You can do so in the code like so:

```python
from filprofiler.api import profile

def main():
    config = load_config()
    result = profile(lambda: run_processing(config), "/tmp/fil-result")
    generate_report(result)
```

You could also make it conditional, e.g. based on an environment variable:

```python
import os
from filprofiler.api import profile

def main():
    config = load_config()
    if os.environ.get("FIL_PROFILE"):
        result = profile(lambda: run_processing(config), "/tmp/fil-result")
    else:
        result = run_processing(config)
    generate_report(result)
```

#### 2. Run your script with Fil

You still need to run your program in a special way.
If previously you did:

```console
$ python yourscript.py --config=myconfig
```

Now you would do:

```console
$ fil-profile python yourscript.py --config=myconfig
```

Notice that you're doing `fil-profile `**`python`**, rather than `fil-profile run` as you would if you were profiling the full script.
Only functions running for the duration of the `filprofiler.api.profile()` call will have memory profiling enabled, including of course the function you pass in.
The rest of the code will run at (close) to normal speed and configuration.

Each call to `profile()` will generate a separate report.
The memory profiling report will be written to the directory specified as the output destination when calling `profile()`; in or example above that was `"/tmp/fil-result"`.
Unlike full-program profiling:

1. The directory you give will be used directly, there won't be timestamped sub-directories.
   **If there are multiple calls to `profile()`, it is your responsibility to ensure each call writes to a unique directory.**
2. The report(s) will _not_ be opened in a browser automatically, on the presumption you're running this in an automated fashion.
