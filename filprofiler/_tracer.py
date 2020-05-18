"""Trace code, so that libpymemprofile_api know's where we are."""

import atexit
from ctypes import PyDLL
from datetime import datetime
import os
import shlex
import sys
import threading
import webbrowser

from ._utils import library_path

# None effectively means RTLD_NEXT, it seems.
preload = PyDLL(None)
preload.fil_initialize_from_python()


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
    threading.setprofile(None)
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
   function compatRequestFullscreen(elem) {{
       if (elem.requestFullscreen) {{
           return elem.requestFullscreen();
       }} else if (elem.webkitRequestFullscreen) {{
           return elem.webkitRequestFullscreen();
       }} else if (elem.mozRequestFullScreen) {{
           return elem.mozRequestFullScreen();
       }} else if (elem.msRequestFullscreen) {{
           return elem.msRequestFullscreen();
       }}
   }}
   function fullScreen(id) {{
       var elem = document.querySelector(id);
       var currentHeight = elem.style.height;
       elem.style.height = "100%";
       compatRequestFullscreen(elem).finally(
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

<h2>Understanding the graphs</h2>
<p>The flame graphs shows the callstacks responsible for allocations at peak.</p>

<p>The wider (and the redder) the bar, the more memory was allocated by that function or its callers.
If the bar is 100% of width, that's all the allocated memory.</p>

<p>The first graph shows the normal callgraph: if <tt>main()</tt> calls <tt>g()</tt> calls <tt>f()</tt>, let's say, then <tt>main()</tt> will be at the top.
The second graph shows the reverse callgraph, from <tt>f()</tt> upwards.</p>

<p>Why is the second graph useful? If <tt>f()</tt> is called from multiple places, in the first graph it will show up multiple times, at the bottom.
In the second reversed graph all calls to <tt>f()</tt> will be merged together.</p>

<p>Need help reducing your data processing application's memory use? Check out tips and tricks <a href="https://pythonspeed.com/datascience/">here</a>.</p>
</body>
</html>
""".format(
                now=now.ctime(), argv=" ".join(map(shlex.quote, sys.argv))
            )
        )

    print("=fil-profile= Wrote HTML report to " + index_path, file=sys.stderr)
    try:
        webbrowser.open(index_path)
    except webbrowser.Error:
        print(
            "=fil-profile= Failed to open browser. You can find the new run at:",
            file=sys.stderr,
        )
        print("=fil-profile= " + index_path, file=sys.stderr)


def trace(code, globals_, output_path: str):
    """
    Given code (Python or code object), run it under the tracer until the
    program exits.
    """
    # Use atexit rather than try/finally so threads that live beyond main
    # thread also get profiled:
    atexit.register(stop_tracing, output_path)
    start_tracing()
    exec(code, globals_, None)
