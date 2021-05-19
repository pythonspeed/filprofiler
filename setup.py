from os import environ, getcwd
from os.path import join
from glob import glob
import sys

from setuptools import setup
from setuptools_rust import RustExtension, Binding

from cflags import CFLAGS


def read(path):
    with open(path) as f:
        return f.read()


# Will be used by filpreload/build.rs's usage of cc to compile C code that uses
# Python APIs.
environ["CFLAGS"] = CFLAGS

# Set public symbols to use for macOS. For some reason this doesn't work in
# build.rs.
if sys.platform.startswith("darwin"):
    environ[
        "RUSTFLAGS"
    ] = f"-C link-arg=-Wl,-exported_symbols_list,{getcwd()}/filpreload/export_symbols.txt"

setup(
    name="filprofiler",
    packages=["filprofiler"],
    entry_points={
        "console_scripts": ["fil-profile=filprofiler._script:stage_1"],
    },
    package_data={
        "filprofiler": ["licenses.txt"],
    },
    data_files=[
        (
            join("share", "jupyter", "kernels", "filprofile"),
            glob(join("data_kernelspec", "*")),
        ),
    ],
    rust_extensions=[
        RustExtension(
            "filprofiler._filpreload",
            path="filpreload/Cargo.toml",
            debug=False,
            binding=Binding.PyO3,
        )
    ],
    use_scm_version=True,
    install_requires=["threadpoolctl"],
    extras_require={"dev": read("requirements-dev.txt").strip().splitlines()},
    setup_requires=["setuptools_scm", "setuptools-rust"],
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
