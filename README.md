# The Fil memory profiler for Python

Fil a memory profiler designed for data processing applications.

Your code reads some data, processes it, and—uses too much memory.
What you need to reduce is _peak_ memory usage.

And that's exactly what Fil will help you find: exactly which code was responsible for allocating memory at _peak_ memory usage.

To install:

```
$ pip install filprofiler
```

To use, instead of doing:

```
$ python yourscript.py --input-file=yourfile
```

Just do:

```
$ fil-profile yourscript.py --input-file=yourfile
```

For more information see https://pythonspeed.com/products/filprofiler/

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
