from setuptools import setup, Extension

try:
    from wheel.bdist_wheel import bdist_wheel as _bdist_wheel

    class bdist_wheel(_bdist_wheel):
        def finalize_options(self):
            _bdist_wheel.finalize_options(self)
            self.root_is_pure = False


except ImportError:
    bdist_wheel = None

setup(
    name="filprofiler",
    packages=["filprofiler"],
    entry_points={"console_scripts": ["fil-profile=filprofiler._script:stage_1"],},
    # TODO dynlib on mac
    package_data={"filprofiler": ["_filpreload.so"]},
    use_scm_version=True,
    setup_requires=["setuptools_scm"],
    extras_require={
        "dev": ["pytest", "pampy", "numpy", "scikit-image", "cython", "black"],
    },
    # Force binary wheels:
    cmdclass={"bdist_wheel": bdist_wheel},
)
