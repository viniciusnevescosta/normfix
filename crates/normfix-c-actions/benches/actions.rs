//! Benchmarks for the work `normfix` itself does.
//!
//! External tools are deliberately excluded. The official checker and the C
//! compiler dominate a cold run, but their cost is a Python interpreter and a
//! process launch, which say nothing about this code and vary with the machine.
//! What these measure is the part a change here can actually regress: parsing,
//! the token tape, and the fixed-point action scheduler.

// The macros generate undocumented items, and a benchmark is not public API.
#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use normfix_c_actions::{CActionOptions, apply_c_actions};

/// One function shaped like real 42 code: tabs, a declaration block, a blank
/// line before the instructions, and a parenthesized return.
const CLEAN_FUNCTION: &str = concat!(
    "int\tft_process_{n}(int argc, char **argv)\n",
    "{\n",
    "\tint\t\tindex;\n",
    "\tchar\t*value;\n",
    "\n",
    "\tindex = 0;\n",
    "\tvalue = argv[argc - 1];\n",
    "\twhile (index < argc)\n",
    "\t{\n",
    "\t\tif (value[index] == '-')\n",
    "\t\t\treturn (index);\n",
    "\t\tindex++;\n",
    "\t}\n",
    "\treturn (0);\n",
    "}\n",
    "\n",
);

/// The same function as a student first writes it.
const MESSY_FUNCTION: &str = concat!(
    "int ft_process_{n}(int argc,char **argv){\n",
    "  int index;\n",
    "  char *value;\n",
    "  index = 0;\n",
    "  value = argv[argc-1];\n",
    "  while(index<argc){\n",
    "    if(value[index]=='-'){\n",
    "      return index;\n",
    "    }\n",
    "    index++;\n",
    "  }\n",
    "  return 0;\n",
    "}\n",
    "\n",
);

fn build(template: &str, functions: usize) -> String {
    let mut source = String::with_capacity(template.len() * functions);
    for index in 0..functions {
        source.push_str(&template.replace("{n}", &index.to_string()));
    }
    source
}

fn format_source(source: &str) -> usize {
    let options = CActionOptions::default();
    apply_c_actions(camino::Utf8Path::new("bench.c"), source, &[], &options)
        .map_or(0, |result| result.source.len())
}

fn benchmarks(criterion: &mut Criterion) {
    // Isolate the fixed cost paid once per file before any work happens.
    criterion.bench_function("CParser::new", |bencher| {
        bencher.iter(|| normfix_c_syntax::CParser::new().map(|_| ()).is_ok());
    });

    let mut group = criterion.benchmark_group("apply_c_actions");

    // A file the size of a real libft source, already correct: the common case
    // on a repeat run, where the scheduler should find nothing to do.
    let clean_small = build(CLEAN_FUNCTION, 3);
    group.bench_function("clean/50 lines", |bencher| {
        bencher.iter(|| format_source(&clean_small));
    });

    // The same size, but needing every layout action.
    let messy_small = build(MESSY_FUNCTION, 3);
    group.bench_function("messy/40 lines", |bencher| {
        bencher.iter(|| format_source(&messy_small));
    });

    // A large file, where any per-call rebuild of a whole-file index shows up.
    let messy_large = build(MESSY_FUNCTION, 60);
    group.bench_function("messy/800 lines", |bencher| {
        bencher.iter(|| format_source(&messy_large));
    });

    group.finish();
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
