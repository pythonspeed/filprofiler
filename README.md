# The Fil memory profiler for Python

Your Python code reads some data, processes it, and uses too much memory; maybe it even dies due to an out-of-memory error.
In order to reduce memory usage, you first need to figure out:

1. Where peak memory usage is, also known as the high-water mark.
2. What code was responsible for allocating the memory that was present at that peak moment.

That's exactly what Fil will help you find.
Fil an open source memory profiler designed for data processing applications written in Python, and includes native support for Jupyter.
Fil runs on Linux and macOS, and supports Python 3.6 and later.

## Getting help

* For more information, you can **[read the documentation](https://pythonspeed.com/fil/docs/)**.
* If you need help or have any questions, feel free to file an issue or [start a discussion](https://github.com/pythonspeed/filprofiler/discussions).
**When in doubt, please ask—I love questions.**

## What users are saying

> "Within minutes of using your tool, I was able to identify a major memory bottleneck that I never would have thought existed.  The ability to track memory allocated via the Python interface and also C allocation is awesome, especially for my NumPy / Pandas programs."
> 
> —Derrick Kondo

> "Fil has just pointed straight at the cause of a memory issue that's been costing my team tons of time and compute power. Thanks again for such an excellent tool!"
>
> —Peter Sobot

## License

Copyright 2021 Hyphenated Enterprises LLC

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
