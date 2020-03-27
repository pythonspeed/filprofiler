from os.path import join
from setuptools import setup, Extension

setup(
    name="filprofiler",
    packages=["filprofiler"],
    entry_points={"console_scripts": ["fil-profile=filprofiler._script:stage_1"],},
    ext_modules=[
        Extension(
            name="filprofiler._filpreload",
            sources=[join("filprofiler", "_filpreload.c")],
            extra_objects=[join("target", "release", "libpymemprofile_api.a")],
            extra_compile_args=["-fno-omit-frame-pointer"],
            extra_link_args=["-export-dynamic"],
        )
    ],
    use_scm_version=True,
    setup_requires=["setuptools_scm"],
    extras_require={
        "dev": ["pytest", "pampy", "numpy", "scikit-image", "cython", "black"],
    },
)
