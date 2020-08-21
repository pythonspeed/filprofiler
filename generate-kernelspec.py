from os.path import join
from ipykernel.kernelspec import write_kernel_spec, make_ipkernel_cmd

if __name__ == "__main__":
    argv = make_ipkernel_cmd(executable="fil-profile")
    argv.insert(1, "python")
    write_kernel_spec(
        "data_kernelspec", overrides={"argv": argv, "display_name": "Python 3 with Fil"}
    )
