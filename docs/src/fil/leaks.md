# Debugging memory leaks with Fil

Is your program suffering from a memory leak?
You can use Fil to debug it.

Fil works by reporting the moment in your process lifetime where memory is highest.
If your program has a memory leak, eventually the highest memory usage point is always the present, as leaked memory accumulates.

If for example your Python web application is leaking memory, you can:

1. Start it under Fil.
2. Generate lots of traffic that causes memory leaks.
3. When enough memory has leaked that it's noticeable, cleanly kill the process (e.g. Ctrl-C).

Fil will then dump a report that will help pinpoint the leaking code.

For a more in-depth tutorial, read this article on [debugging Python server memory leaks with Fil](https://pythonspeed.com/articles/python-server-memory-leaks/).
