# Using Fil for the first time

First, [install Fil](#installation).

Then, create a Python file called `example.py` with the following code:

```python
import numpy as np

def make_big_array():
    return np.zeros((1024, 1024, 50))

def make_two_arrays():
    arr1 = np.zeros((1024, 1024, 10))
    arr2 = np.ones((1024, 1024, 10))
    return arr1, arr2

def main():
    arr1, arr2 = make_two_arrays()
    another_arr = make_big_array()

main()
```

Now, you can run it with Fil:

```shell-session
$ fil-profile run example.py
```

This will run the program under Fil, and pop up the results.

In the next section, we'll look at the results and see what they tell us.
