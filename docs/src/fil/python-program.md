# Profiling complete Python programs

You want to get a memory profile of your Python program end-to-end, from when it starts running to when it finishes.

## Profiling Python scripts

Let's say you usually run your program like this:

```console
$ python yourscript.py --input-file=yourfile
```

Just do:

```
$ fil-profile run yourscript.py --input-file=yourfile
```

And it will generate a report and automatically try to open it in for you in a browser.
Reports will be stored in the `fil-result/` directory in your current working directory.

You can also use this alternative syntax:

```
$ python -m filprofiler run yourscript.py --input-file=yourfile
```

## Profiling Python modules (`python -m`)

If your program is usually run as a module:

```console
$ python -m yourapp.yourmodule --args
```

You can run it with Fil like this:

```console
$ fil-profile run -m yourapp.yourmodule --args
```

Or like this:

```console
$ python -m filprofiler run -m yourapp.yourmodule --args
```
