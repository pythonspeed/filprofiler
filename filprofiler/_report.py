"""
Report generation and SVG manipulation code.

Eventually this might all be in Rust, but for now it's easier to do some of it
post-fact in Python.
"""

from datetime import datetime
import linecache
import os
import shlex
import re
import sys
from xml.sax.saxutils import escape
from urllib.parse import quote_plus as url_quote
import html
from . import __version__

LINE_REFERENCE = re.compile(r"\<title\>TB@@([^:]+):(\d+)@@TB")

DEBUGGING_INFO = url_quote(
    f"""\
## Version information
Fil: {__version__}
Python: {sys.version}
"""
)


def replace_code_references(string: str) -> str:
    """
    Replace occurrences of TB@@file.py:123@@TB with the line of code at that
    location, XML quoted and slightly indented.
    """

    def replace_with_code(match):
        filename, line = match.group(1, 2)
        filename = html.unescape(filename)
        line = int(line)
        return "<title>&#160;&#160;&#160;&#160;" + escape(
            linecache.getline(filename, line).strip()
        )

    return re.sub(LINE_REFERENCE, replace_with_code, string)


def update_svg(svg_path: str):
    """Fix up the SVGs.

    1. Add an appropriate subtitle.
    2. Add source code lines.
    """
    with open(svg_path) as f:
        data = f.read().replace(
            "SUBTITLE-HERE",
            """Made with the Fil memory profiler. <a href="https://pythonspeed.com/products/filmemoryprofiler/" style="text-decoration: underline;" target="_parent">Try it on your code!</a>""",
        )
        data = replace_code_references(data)
    with open(svg_path, "w") as f:
        f.write(data)


def render_report(output_path: str, now: datetime) -> str:
    """Write out the HTML index and improve the SVGs."""
    for svg_path in [
        os.path.join(output_path, "peak-memory.svg"),
        os.path.join(output_path, "peak-memory-reversed.svg"),
    ]:
        update_svg(svg_path)

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

<h2>Need help, or does something look wrong? <a href="https://github.com/pythonspeed/filprofiler/issues/new?body={bugreport}">Please file an issue</a> and I'll try to help</h2>

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
                now=now.ctime(),
                argv=" ".join(map(shlex.quote, sys.argv)),
                bugreport=DEBUGGING_INFO,
            )
        )
    return index_path
