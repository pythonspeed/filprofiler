"""
Cachegrind performance metrics.
"""

from typing import List, Dict
from subprocess import check_call, check_output
from tempfile import NamedTemporaryFile


def _run(args_list: List[str]) -> Dict[str, int]:
    """
    Run the the given program and arguments under Cachegrind, parse the
    Cachegrind specs.

    For now we just ignore program output, and in general this is not robust.
    """
    ARCH = check_output(["uname", "-m"]).strip()
    temp_file = NamedTemporaryFile("r+")
    check_call(
        [
            # Disable ASLR:
            "setarch",
            ARCH,
            "-R",
            "valgrind",
            "--tool=cachegrind",
            # Set some reasonable L1 and LL values, based on Haswell. You can set
            # your own, important part is that they are consistent across runs,
            # instead of the default of copying from the current machine.
            "--I1=32768,8,64",
            "--D1=32768,8,64",
            "--LL=8388608,16,64",
            "--cachegrind-out-file=" + temp_file.name,
        ]
        + args_list
    )
    return parse_cachegrind_output(temp_file)


def parse_cachegrind_output(temp_file):
    # Parse the output file:
    lines = iter(temp_file)
    for line in lines:
        if line.startswith("events: "):
            header = line[len("events: ") :].strip()
            break
    for line in lines:
        last_line = line
    assert last_line.startswith("summary: ")
    last_line = last_line[len("summary:") :].strip()
    return dict(zip(header.split(), [int(i) for i in last_line.split()]))


def get_counts(cg_results: Dict[str, int]) -> Dict[str, int]:
    """
    Given the result of _run(), figure out the parameters we will use for final
    estimate.

    We pretend there's no L2 since Cachegrind doesn't currently support it.

    Caveats: we're not including time to process instructions, only time to
    access instruction cache(s), so we're assuming time to fetch and run
    instruction is the same as time to retrieve data if they're both to L1
    cache.
    """
    result = {}
    d = cg_results

    ram_hits = d["DLmr"] + d["DLmw"] + d["ILmr"]

    l3_hits = d["I1mr"] + d["D1mw"] + d["D1mr"] - ram_hits

    total_memory_rw = d["Ir"] + d["Dr"] + d["Dw"]
    l1_hits = total_memory_rw - l3_hits - ram_hits
    assert total_memory_rw == l1_hits + l3_hits + ram_hits

    result["l1"] = l1_hits
    result["l3"] = l3_hits
    result["ram"] = ram_hits

    return result


def combined_instruction_estimate(counts: Dict[str, int]) -> int:
    """
    Given the result of _run(), return estimate of total time to run.

    Multipliers were determined empirically, but some research suggests they're
    a reasonable approximation for cache time ratios.  L3 is probably too low,
    but then we're not simulating L2...
    """
    return counts["l1"] + (5 * counts["l3"]) + (35 * counts["ram"])


def benchmark(args_list: List[str]) -> Dict[str, int]:
    """Run the program, return dictionary with raw data and summary metric."""
    result = _run(args_list)
    counts = get_counts(result)
    result["Overall"] = combined_instruction_estimate(counts)
    return result
