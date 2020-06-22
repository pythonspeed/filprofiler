# The Fil memory profiler for Python

Fil a memory profiler designed for data processing applications.
At the moment it only runs on Linux and macOS.

Your code reads some data, processes it, and—uses too much memory.
What you need to reduce is _peak_ memory usage.

And that's exactly what Fil will help you find: exactly which code was responsible for allocating memory at _peak_ memory usage.

For more information see https://pythonspeed.com/products/filmemoryprofiler/

## Installation

To install:

```
$ pip install filprofiler
```

## Measuring peak (high-water mark) memory usage

Instead of doing:

```
$ python yourscript.py --input-file=yourfile
```

Just do:

```
$ fil-profile run yourscript.py --input-file=yourfile
```

And it will generate a report.

## Debugging out-of-memory crashes

First, run `free` to figure out how much memory is available—in this case about 6.3GB—and then set a corresponding limit on virtual memory with `ulimit`:

```shell
$ free -h
       total   used   free  shared  buff/cache  available
Mem:   7.7Gi  1.1Gi  6.3Gi    50Mi       334Mi      6.3Gi
Swap:  3.9Gi  3.0Gi  871Mi
$ ulimit -Sv 6300000
```

Then, run your program under Fil, and it will generate a SVG at the point in time when memory runs out:

```shell
$ fil-profile run oom.py 
...
=fil-profile= Wrote memory usage flamegraph to fil-result/2020-06-15T12:37:13.033/out-of-memory.svg
```

## You've found where memory usage is coming from—now what?

If you're using data processing or scientific computing libraries, I have written a relevant [guide to reducing memory usage](https://pythonspeed.com/datascience/).

## License

Copyright 2020 Hyphenated Enterprises LLC

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
