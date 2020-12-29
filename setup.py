from os import environ
from os.path import join
from glob import glob
from setuptools import setup, Extension
from distutils import sysconfig
import sys


if sys.platform == "darwin":
    # Want a dynamiclib so that it can inserted with DYLD_INSERT_LIBRARIES:
    config_vars = sysconfig.get_config_vars()
    config_vars["LDSHARED"] = config_vars["LDSHARED"].replace("-bundle", "-dynamiclib")


def read(path):
    with open(path) as f:
        return f.read()


extra_compile_args = ["-fno-omit-frame-pointer"]
if environ.get("CONDA_PREFIX"):
    extra_compile_args.append("-DFIL_SKIP_ALIGNED_ALLOC=1")


setup(
    name="filprofiler",
    packages=["filprofiler"],
    entry_points={"console_scripts": ["fil-profile=filprofiler._script:stage_1"],},
    ext_modules=[
        Extension(
            name="filprofiler._filpreload",
            sources=[join("filprofiler", "_filpreload.c")],
            extra_objects=[join("target", "release", "libpymemprofile_api.a")],
            extra_compile_args=extra_compile_args,
            extra_link_args=["-export-dynamic"],
        )
    ],
    package_data={"filprofiler": ["licenses.txt"],},
    data_files=[
        (
            join("share", "jupyter", "kernels", "filprofile"),
            glob(join("data_kernelspec", "*")),
        ),
    ],
    use_scm_version=True,
    install_requires=["threadpoolctl"],
    setup_requires=["setuptools_scm"],
    extras_require={"dev": read("requirements-dev.txt").strip().splitlines()},
    description="A memory profiler for data batch processing applications.",
    classifiers=[
        "Intended Audience :: Developers",
        "License :: OSI Approved :: Apache Software License",
        "Operating System :: MacOS",
        "Operating System :: POSIX :: Linux",
        "Programming Language :: Python",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.6",
        "Programming Language :: Python :: 3.7",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: Implementation :: CPython",
    ],
    python_requires=">=3.6",
    license="Apache 2.0",
    url="https://pythonspeed.com/products/filmemoryprofiler/",
    maintainer="Itamar Turner-Trauring",
    maintainer_email="itamar@pythonspeed.com",
    long_description=read("README.md"),
    long_description_content_type="text/markdown",
)
