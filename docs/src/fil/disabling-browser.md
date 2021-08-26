# Disabling browser pop-up reports

By default, Fil will open the result of a profiling run in a browser.

As of version 2021.04.2, you can disable this by using the `--no-browser` option (see `fil-profile --help` for details).
You will want to view the SVG report in a browser, since they rely heavily on JavaScript.

If you want to serve the report files from a static directory using a web server, you can do:

```console
$ cd fil-result/
$ python -m http.server
```

