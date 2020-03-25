"""Trace code, so that libpymemprofile_api.so know's where we are."""

import atexit
from ctypes import PyDLL
from datetime import datetime
import os
import shlex
import sys
import threading
import webbrowser

from ._utils import library_path

# Load with RTLD_GLOBAL so _profiler.so has access to those symbols; explicit
# linking may be possible but haven't done that yet, oh well.
# pymemprofile = CDLL(library_path("libpymemprofile_api"), mode=RTLD_GLOBAL)
preload = PyDLL(None)  # the executable


def start_tracing():
    preload.fil_reset()
    threading.setprofile(_start_thread_trace)
    preload.register_fil_tracer()


def _start_thread_trace(frame, event, arg):
    """Trace function that can be passed to sys.settrace.

    All this does is register the underlying C trace function, using the
    mechanism described in
    https://github.com/nedbat/coveragepy/blob/master/coverage/ctracer/tracer.c's
    CTracer_call.
    """
    if event == "call":
        preload.register_fil_tracer()
    return _start_thread_trace


def stop_tracing(output_path: str):
    sys.setprofile(None)
    dump_svg(output_path)
    preload.fil_shutting_down()


def dump_svg(output_path: str):
    now = datetime.now()
    output_path = os.path.join(output_path, now.isoformat(timespec="milliseconds"))
    path = output_path.encode("utf-8")
    preload.fil_dump_peak_to_flamegraph(path)
    for svg_path in [
        os.path.join(output_path, "peak-memory.svg"),
        os.path.join(output_path, "peak-memory-reversed.svg"),
    ]:
        with open(svg_path) as f:
            data = f.read().replace(
                "SUBTITLE-HERE",
                """Made with the Fil memory profiler. <a href="https://pythonspeed.com/products/filmemoryprofiler/" style="text-decoration: underline;" target="_parent">Try it on your code!</a>""",
            )
            with open(svg_path, "w") as f:
                f.write(data)
    index_path = os.path.join(output_path, "index.html")
    with open(index_path, "w") as index:
        index.write(
            """
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Fil Memory Profile ({now})</title>
  <style type="text/css">
    body {{
        font-family: -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Oxygen-Sans,Ubuntu,Cantarell,"Helvetica Neue",sans-serif;
        line-height: 1.2;
        max-width: 40rem;
        margin: 4rem auto;
        font-size: 18px;
    }}
    div {{
        text-align: center;
    }}
  </style>
  <script>
   function fullScreen(id) {{
       var elem = document.querySelector(id);
       var currentHeight = elem.style.height;
       elem.style.height = "100%";
       elem.requestFullscreen().finally(
           (info) => {{elem.style.height = currentHeight;}}
       );
   }}
  </script>
</head>
<body>
<h1>Fil Memory Profile</h1>
<h2>{now}</h2>
<h2>Command</h2>
<p><code>{argv}</code><p>

<h2>Profiling result</h2>
<div><iframe id="peak" src="peak-memory.svg" width="100%" height="200" scrolling="auto" frameborder="0"></iframe><br>
<p><input type="button" onclick="fullScreen('#peak');" value="Full screen"></p></div>

<br>

<div><iframe id="peak-reversed" src="peak-memory-reversed.svg" width="100%" height="200" scrolling="auto" frameborder="0"></iframe><br>
<p><input type="button" onclick="fullScreen('#peak-reversed');" value="Full screen"></p></div>
            </div>
""".format(
                now=now.ctime(), argv=" ".join(map(shlex.quote, sys.argv))
            )
        )

    try:
        webbrowser.open(index_path)
    except webbrowser.Error:
        print(
            "=fil-profile= Failed to open browser. You can find the new run at:",
            file=sys.stderr,
        )
        print("=fil-profile= " + index_path, fil=sys.stderr)


def trace(code, globals_, output_path: str):
    """
    Given code (Python or code object), run it under the tracer until the
    program exits.
    """
    atexit.register(stop_tracing, output_path)
    start_tracing()
    exec(code, globals_, None)
