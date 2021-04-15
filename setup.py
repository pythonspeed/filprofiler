from os import environ
from os.path import join
from glob import glob
from setuptools import setup, Extension
from distutils import sysconfig
import sys


def read(path):
    with open(path) as f:
        return f.read()


extra_compile_args = ["-fno-omit-frame-pointer"]
extra_link_args = ["-export-dynamic"]
if sys.platform == "darwin":
    # Want a dynamiclib so that it can inserted with DYLD_INSERT_LIBRARIES:
    config_vars = sysconfig.get_config_vars()
    config_vars["LDSHARED"] = config_vars["LDSHARED"].replace("-bundle", "-dynamiclib")
else:
    # Linux
    extra_link_args.extend(
        [
            # Indicate which symbols are public. macOS lld doesn't support version
            # scripts.
            "-Wl,--version-script=versionscript.txt",
            # Make sure aligned_alloc() is public under its real name;
            # workaround for old glibc headers in Conda.
            "-Wl,--defsym=aligned_alloc=reimplemented_aligned_alloc",
            # On 64-bit Linux, mmap() is another way of saying mmap64, or vice
            # versa, so we point to function of our own.
            "-Wl,--defsym=mmap=fil_mmap_impl",
            "-Wl,--defsym=mmap64=fil_mmap_impl",
        ]
    )


setup(
    name="filprofiler",
    packages=["filprofiler"],
    entry_points={
        "console_scripts": ["fil-profile=filprofiler._script:stage_1"],
    },
    ext_modules=[
        Extension(
            name="filprofiler._filpreload",
            sources=[join("filprofiler", "_filpreload.c")],
            extra_objects=[join("target", "release", "libfilpreload.a")],
            extra_compile_args=extra_compile_args,
            extra_link_args=extra_link_args,
        )
    ],
    package_data={
        "filprofiler": ["licenses.txt"],
    },
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
