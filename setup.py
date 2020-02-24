from setuptools import setup, Extension

setup(
    name="filprofiler",
    version="0.1",
    packages=["filprofiler"],
    ext_modules=[
        Extension("filprofiler._profiler", sources=["filprofiler/_profiler.c"],)
    ],
    entry_points={"console_scripts": ["fil-profile=filprofiler._script:stage_1"],},
    package_data={
        "filprofiler": [
            # TODO dynlib on Macs.
            "libpymemprofile_api.so",
            "_filpreload.so",
        ]
    },
)
