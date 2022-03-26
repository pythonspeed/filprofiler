# Understanding Fil's output

Let's look at the result of the Fil run from the previous section:

<p align="center"><input type="button" onclick="fullScreen('#graph');" value="Click to view graph with full screen ⬎"></p>

<iframe src="memory-graph.svg" id="graph" width="100%" height="450" scrolling="auto" frameborder="0"></iframe>
<br>

What does this mean?

What you're seeing is a _flamegraph_, a visualization that shows a tree of callstacks and which ones were most expensive.
In Fil's case, it shows the callstacks responsible for memory allocations at the point in time when memory usage was highest.

The wider or redder the frame, the higher percentage of memory that function was responsible for.
Each line is an additional call in the callstack.

This particular flamegraph is interactive:

* **Click on a frame** to see a zoomed in view of that part of the callstack.
  You can then **click "Reset zoom"** in the upper left corner to get back to the main overview.
* **Hover over a frame** with your mouse to get additional details.

**To optimize your code, focus on the wider and redder frames.**
These are the frames that allocated most of the memory.
In this particular example, you can see that the most memory was allocated by a line of code in the `make_big_array()` function.

Having found the source of the memory allocations at the moment of peak memory usage, you can then go and [reduce memory usage](https://pythonspeed.com/fil/).
You can then validate your changes reduced memory usage by re-running your updated program with Fil and comparing the result.
