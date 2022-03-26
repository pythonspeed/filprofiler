# Installing Fil

Fil requires macOS or Linux, and Python 3.7 or Later.
You can either use Conda, a sufficiently new version of Pip, or higher-level tools like Poetry or Pipenv.

## Conda

To install on Conda:

```console
$ conda install -c conda-forge filprofiler
```

## Pip (or similar tools)

To install the latest version of Fil you'll need Pip 19 or newer.
You can check the current version like this:

```console
$ pip --version
pip 19.3.0
```

If you're using something older than v19, you can upgrade by doing:

```console
$ pip install --upgrade pip
```

If _that_ doesn't work, try running your code in a virtualenv (always a good idea in general):

```console
$ python3 -m venv venv/
$ source venv/bin/activate
(venv) $ pip install --upgrade pip
```

Assuming you have a new enough version of pip, you can now install Fil:

```console
$ pip install filprofiler
```
