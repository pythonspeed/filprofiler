from setuptools import setup, Extension

setup(
    name="filprofiler",
    version="0.1",
    packages=["filprofiler"],
    entry_points={"console_scripts": ["fil-profile=filprofiler._script:stage_1"],},
    package_data={
        "filprofiler": [
            # TODO dynlib on Macs.
            "filprofiler/libpymemprofile_api.so",
            "filprofiler/_filpreload.so",
        ]
    },
)
