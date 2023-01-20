"""
Report generation and SVG manipulation code.

Eventually this might all be in Rust, but for now it's easier to do some of it
post-fact in Python.
"""

from datetime import datetime
import os
import shlex
import sys
from urllib.parse import quote_plus as url_quote
from . import __version__

DEBUGGING_INFO = url_quote(
    f"""\
## Version information
Fil: {__version__}
Python: {sys.version}
"""
)


def render_report(output_path: str, now: datetime) -> str:
    """Write out the HTML index and improve the SVGs."""
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
        max-width: 90%;
        margin: 4rem auto;
        font-size: 18px;
    }}
    .center {{
       max-width: 40rem;
       margin: 0 auto;
    }}
    blockquote {{ border-width: 1px; border-color: black; border-style: solid; padding: 1em; }}
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
<div style="text-align: center;"><p><input type="button" onclick="fullScreen('#peak');" value="Full screen"> · <a href="peak-memory.svg" target="_blank"><button>Open in new window</button></a></p>
<iframe id="peak" src="peak-memory.svg" width="100%" height="700" scrolling="auto" frameborder="0"></iframe>
</div>
<br>
<blockquote class="center">
            <h3>Find performance bottlenecks in your data processing jobs with the Sciagraph profiler</h3>
            <p><strong><a href="https://sciagraph.com/">The Sciagraph profiler</a></strong> can help you
            <strong>find performance
            and memory bottlenecks with low overhead, so you can use it in both development and production.</strong></p>
            <p>Unlike Fil, it includes performance profiling. Sciagraph's memory profiling uses sampling so it runs faster than Fil, but unlike Fil
            it can't accurately profile small allocations or run natively on macOS.</p></blockquote>
<br>
            <br>
<div style="text-align: center;"><p><input type="button" onclick="fullScreen('#peak-reversed');" value="Full screen"> ·
<a href="peak-memory-reversed.svg" target="_blank"><button>Open in new window</button></a></p>
            <iframe id="peak-reversed" src="peak-memory-reversed.svg" width="100%" height="400" scrolling="auto" frameborder="0"></iframe><br>
</div>

<div class="center">
<blockquote><strong>Need help, or does something look wrong?</strong>
<a href="https://pythonspeed.com/fil/docs/">Read the documentation</a>,
and if that doesn't help please
<a href="https://github.com/pythonspeed/filprofiler/issues/new?body={bugreport}">file an issue</a>
and I'll try to help.</blockquote>
<br>
<h2>Learn how to reduce memory usage</h2>

<p>Need help reducing your data processing application's memory use? Check out tips and tricks <a href="https://pythonspeed.com/memory/">here</a>.</p>

<h2>Understanding the graphs</h2>
<p>The flame graphs shows the callstacks responsible for allocations at peak.</p>

<p>The wider (and the redder) the bar, the more memory was allocated by that function or its callers.
If the bar is 100% of width, that's all the allocated memory.</p>

<p>The left-right axis has no meaning!
The order of frames is somewhat arbitrary, for example beause multiple calls to the same function may well have been merged into a single callstack.
So you can't tell from the graph which allocations happened first.
All you're getting is that at peak allocation these time, these stacktraces were responsible for these allocations.
</p>

<p>The first graph shows the normal callgraph: if <tt>main()</tt> calls <tt>g()</tt> calls <tt>f()</tt>, let's say, then <tt>main()</tt> will be at the top.
The second graph shows the reverse callgraph, from <tt>f()</tt> upwards.</p>

<p>Why is the second graph useful? If <tt>f()</tt> is called from multiple places, in the first graph it will show up multiple times, at the bottom.
In the second reversed graph all calls to <tt>f()</tt> will be merged together.</p>

<h2>Understanding what Fil tracks</h2>

<p>Fil measures how much memory has been allocated; this is not the same as how much memory the process is actively using, nor is it the same as memory resident in RAM.</p>

<ul>
<li>If the data gets dumped from RAM to swap, Fil still counts it but it's not counted as resident in RAM.</li>
<li>If the memory is a large chunk of all zeros, on Linux no RAM is used by OS until you actually modify that memory, but Fil will still count it.</li>
<li>If you have memory that only gets freed on garbage collection
(this will happen if you have circular references in your data structures),
memory can be freed at inconsistent times across different runs, especially
if you're using threads.</li>
</ul>

<p>See <a href="https://pythonspeed.com/articles/measuring-memory-python/">this article</a> for more details.</p>
</div>
</body>
</html>
""".format(
                now=now.ctime(),
                argv=" ".join(map(shlex.quote, sys.argv)),
                bugreport=DEBUGGING_INFO,
            )
        )
    return index_path
