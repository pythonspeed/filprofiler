from setuptools import setup, Extension

setup(
    name="filprofiler",
    version="0.1",
    packages=["filprofiler"],
    entry_points={"console_scripts": ["fil-profile=filprofiler._script:stage_1"],},
    package_data={"filprofiler": ["fil-python",]},
    use_scm_version=True,
    setup_requires=["setuptools_scm"],
    extras_require={
        "dev": ["pytest", "pampy", "numpy", "scikit-image", "cython", "black"],
    },
)
